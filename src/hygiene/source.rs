use super::{Report, Settings, json};
use crate::model::{Error, Result, WithCode};
use crate::process;
use crate::repo::Repo;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Facts {
    pub edges: BTreeMap<String, usize>,
    pub module_edges: BTreeMap<String, usize>,
    pub functions: Vec<Function>,
    pub findings: Vec<(String, usize, String)>,
    pub guards: Vec<String>,
    pub parse_error: Option<String>,
    pub exports: u64,
    pub code_lines: u64,
    pub comment_lines: u64,
    pub commented_code: Vec<usize>,
}

#[derive(Deserialize)]
pub struct Function {
    pub name: String,
    pub line: usize,
    pub lines: u64,
    pub tokens: Vec<String>,
}

pub fn typescript(
    repo: &Repo,
    production: &BTreeMap<String, String>,
    tracked: &BTreeSet<&str>,
    policy: &Settings,
    report: &mut Report,
) -> Result<BTreeMap<String, Facts>> {
    let settings = serde_json::to_string(&policy.ui).code("HYGIENE_CONFIG")?;
    let temp = tempfile::NamedTempFile::new().code("HYGIENE_INPUT")?;
    let tests = tracked
        .iter()
        .copied()
        .filter(|path| {
            policy.evidence.contains_key("S14")
                && crate::integrity::test_path(path)
                && path.rsplit_once('.').is_some_and(|(_, ext)| {
                    matches!(
                        ext,
                        "js" | "jsx" | "cjs" | "mjs" | "ts" | "tsx" | "cts" | "mts"
                    )
                })
        })
        .collect::<BTreeSet<_>>();
    let files = serde_json::json!({
        "production": production.keys().collect::<Vec<_>>(),
        "tests": tests,
    });
    serde_json::to_writer(temp.as_file(), &files).code("HYGIENE_INPUT")?;
    let path = temp
        .path()
        .to_str()
        .ok_or_else(|| Error::new("HYGIENE_INPUT", "Cannot pass non-UTF-8 input path to Node"))?;
    let output = process::run(
        &repo.root,
        "node",
        &["-e", include_str!("typescript.cjs"), &settings, path],
        Duration::from_secs(120),
    )?
    .checked("TypeScript syntax analysis")?;
    let mut facts: BTreeMap<String, Facts> = json(&output)?;
    for path in tests {
        let file = facts.remove(path).ok_or_else(|| {
            Error::new(
                "HYGIENE_OUTPUT",
                format!("Missing test source analysis for {path}"),
            )
        })?;
        for guard in file.guards {
            *report.guards.entry(format!("{path}:{guard}")).or_default() += 1;
        }
    }
    if !facts.keys().eq(production.keys()) {
        return Err(Error::new(
            "HYGIENE_OUTPUT",
            "TypeScript syntax analysis changed its input scope",
        ));
    }
    Ok(facts)
}

pub fn summarize(facts: &BTreeMap<String, Facts>, limit: u32, report: &mut Report) {
    let mut functions: BTreeMap<&[String], (&str, usize)> = BTreeMap::new();
    let mut duplicated = 0;
    let mut measured = 0;
    let mut depth = BTreeMap::new();
    let mut comments = BTreeMap::new();
    for (path, file) in facts {
        if let Some(error) = &file.parse_error {
            report
                .unavailable
                .insert("source-analysis".into(), format!("{path}: {error}"));
        }
        for guard in &file.guards {
            *report.guards.entry(format!("{path}:{guard}")).or_default() += 1;
        }
        for (id, line, detail) in &file.findings {
            report.record(id, path, *line, detail, 1);
        }
        for function in &file.functions {
            let excess = function.lines.saturating_sub(limit.into());
            report.record("S19", path, function.line, &function.name, excess);
            measured += function.lines;
            if function.tokens.len() < 64 {
                continue;
            }
            if let Some((other, line)) = functions.get(function.tokens.as_slice()) {
                duplicated += function.lines;
                report.record(
                    "S11",
                    path,
                    function.line,
                    format!("Identical function body at {other}:{line}"),
                    function.lines,
                );
            } else {
                functions.insert(&function.tokens, (path, function.line));
            }
        }
        if file.exports > 0 {
            depth.insert(path, [file.code_lines, file.exports]);
        }
        if file.comment_lines > 0 {
            comments.insert(
                path,
                serde_json::json!({
                    "comment_lines": file.comment_lines,
                    "code_lines": file.code_lines,
                    "commented_statement_lines": file.commented_code
                }),
            );
        }
    }
    report.inventory.insert("S11".into(), serde_json::json!({"duplicate_function_lines": duplicated, "measured_function_lines": measured}));
    report
        .inventory
        .insert("S18".into(), serde_json::json!(depth));
    report
        .inventory
        .insert("S20".into(), serde_json::json!(comments));
}

pub fn comment(facts: &mut Facts, line: usize, text: &str) {
    facts.comment_lines += text.lines().count().max(1) as u64;
    for (offset, text) in text.lines().enumerate() {
        let content = text
            .trim_start_matches(['/', '*', '!', ' ', '\t'])
            .trim_end();
        if content.starts_with("let ")
            && content.ends_with(';')
            && syn::parse_str::<syn::Stmt>(content)
                .is_ok_and(|statement| matches!(statement, syn::Stmt::Local(_)))
        {
            facts.commented_code.push(line + offset);
        }
        let words: BTreeSet<_> = text
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .collect();
        if ["TODO", "FIXME", "HACK", "XXX"]
            .iter()
            .any(|tag| words.contains(tag))
        {
            facts
                .findings
                .push(("S13".into(), line + offset, "Maintenance marker".into()));
        }
        if text
            .to_ascii_lowercase()
            .contains("in a real implementation")
        {
            facts
                .findings
                .push(("S24".into(), line + offset, "Placeholder comment".into()));
        }
    }
}
