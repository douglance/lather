use super::{Report, json};
use crate::model::{Policy, Result, WithCode};
use crate::repo::Repo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

pub fn measure(
    repo: &Repo,
    tracked: &BTreeSet<&str>,
    policy: &Policy,
    report: &mut Report,
) -> Result<()> {
    let mut custom = Vec::new();
    let mut workflows = Vec::new();
    let mut e2e = Vec::new();
    let mut hooks = Vec::new();
    let mut verify = Vec::new();
    let mut tooling = BTreeMap::new();
    let docs = ["AGENTS.md", "CLAUDE.md", "CODING_STANDARDS.md"];
    for &path in tracked {
        let name = path.rsplit('/').next().unwrap_or(path);
        if path.starts_with("hygiene-checks/")
            || path.starts_with("ci/")
            || path.starts_with("eslint-rules/")
            || path.starts_with("ast-grep-rules/")
            || matches!(
                name,
                "deny.toml" | ".ast-grep.yml" | "sgconfig.yml" | "sgconfig.yaml"
            )
        {
            custom.push(path);
        }
        if path.starts_with(".github/workflows/") {
            let text = read(repo, path)?.unwrap_or_default();
            workflows.push((path, workflow_checks(&text)));
        }
        if path
            .split('/')
            .any(|part| matches!(part, "e2e" | "playwright" | "cypress" | "simulator"))
            || name.ends_with(".stories.tsx")
            || name.ends_with(".stories.jsx")
        {
            e2e.push(path);
        }
        if path.starts_with(".husky/")
            || matches!(
                name,
                "lefthook.yml" | "lefthook.yaml" | ".pre-commit-config.yaml"
            )
        {
            hooks.push(path);
        }
        if matches!(name, "Makefile" | "justfile" | "Justfile") {
            let text = read(repo, path)?.unwrap_or_default();
            if text.lines().any(|line| {
                ["verify:", "check:"]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            }) {
                verify.push(path);
            }
        }
        formatters(path, name, &mut tooling);
        if matches!(name, "Cargo.toml" | "package.json")
            && let Some(text) = read(repo, path)?
        {
            manifest(path, &text, report)?;
        }
    }
    for configs in tooling.values() {
        for &path in configs.iter().skip(1) {
            report.record(
                "S21",
                path,
                1,
                "Multiple formatter configurations in this directory",
                1,
            );
        }
    }
    report.inventory.insert("S6".into(), serde_json::json!({"tracked_candidates": custom, "evidence_commands": policy.hygiene.evidence.keys().collect::<Vec<_>>()}));
    report.inventory.insert("S17".into(), serde_json::json!({"workflows": workflows, "e2e_or_story_paths": e2e, "hook_configs": hooks, "verify_targets": verify}));
    instructions(repo, tracked, &docs, custom.len(), report)?;
    Ok(())
}

fn formatters<'a>(path: &'a str, name: &str, tooling: &mut BTreeMap<&'a str, Vec<&'a str>>) {
    if name.starts_with(".prettierrc")
        || name.starts_with("prettier.config.")
        || matches!(
            name,
            "rustfmt.toml"
                | ".rustfmt.toml"
                | "biome.json"
                | "biome.jsonc"
                | ".oxfmtrc.json"
                | ".oxfmtrc.jsonc"
        )
    {
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        tooling.entry(parent).or_default().push(path);
    }
}

fn manifest(path: &str, text: &str, report: &mut Report) -> Result<()> {
    let mut names = BTreeSet::new();
    if path.ends_with("Cargo.toml") {
        let config: toml::Value = toml::from_str(text).code("HYGIENE_MANIFEST")?;
        for table in [config.get("dependencies"), config.get("dev-dependencies")]
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_table)
        {
            names.extend(table.iter().map(|(name, value)| {
                value
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(name)
            }));
        }
        conventions(
            path,
            &names,
            &[
                ("HTTP client", &["reqwest", "ureq", "isahc"]),
                ("date and time", &["chrono", "time", "jiff"]),
                ("async runtime", &["tokio", "async-std", "smol"]),
            ],
            report,
        );
    } else {
        let config: serde_json::Value = json(text)?;
        for field in ["dependencies", "devDependencies"] {
            if let Some(dependencies) = config[field].as_object() {
                names.extend(dependencies.keys().map(String::as_str));
            }
        }
        conventions(
            path,
            &names,
            &[
                ("HTTP client", &["axios", "got", "ky", "node-fetch"]),
                ("date and time", &["moment", "dayjs", "date-fns", "luxon"]),
                (
                    "state management",
                    &[
                        "redux",
                        "@reduxjs/toolkit",
                        "zustand",
                        "jotai",
                        "recoil",
                        "mobx",
                    ],
                ),
                ("test runner", &["jest", "vitest", "mocha", "ava", "tap"]),
            ],
            report,
        );
        if let Some(scripts) = config["scripts"].as_object() {
            let mut verify: Vec<_> = scripts
                .keys()
                .filter(|name| {
                    matches!(name.as_str(), "verify" | "check" | "validate" | "test:e2e")
                })
                .collect();
            verify.sort_unstable();
            if !verify.is_empty() {
                report
                    .inventory
                    .insert(format!("S17.scripts.{path}"), serde_json::json!(verify));
            }
        }
    }
    Ok(())
}

fn conventions(
    path: &str,
    names: &BTreeSet<&str>,
    families: &[(&str, &[&str])],
    report: &mut Report,
) {
    for (family, packages) in families {
        let present: Vec<_> = packages
            .iter()
            .filter(|name| names.contains(*name))
            .copied()
            .collect();
        // Redux Toolkit and Redux may intentionally coexist as one stack.
        let overlap =
            usize::from(present.contains(&"redux") && present.contains(&"@reduxjs/toolkit"));
        let debt = present.len().saturating_sub(1 + overlap);
        if debt > 0 {
            report.record(
                "S21",
                path,
                1,
                format!("Multiple {family} packages: {}", present.join(", ")),
                debt as u64,
            );
        }
    }
}

fn instructions(
    repo: &Repo,
    tracked: &BTreeSet<&str>,
    docs: &[&str],
    custom_count: usize,
    report: &mut Report,
) -> Result<()> {
    let mut prose = 0;
    let mut markdown = BTreeMap::new();
    for &name in docs.iter().filter(|&&name| tracked.contains(name)) {
        if let Some(body) = read(repo, name)? {
            prose += prose_rules(&body);
            markdown.insert(name, instruction_facts(&body, tracked));
        } else if name == "CLAUDE.md"
            && std::fs::read_link(repo.root.join(name))
                .is_ok_and(|path| path == Path::new("AGENTS.md"))
        {
            markdown.insert(name, serde_json::json!({"points_to_agents": true, "target_tracked": tracked.contains("AGENTS.md")}));
        }
    }
    report.inventory.insert(
        "S7".into(),
        serde_json::json!({"imperative_bullets": prose, "denominator": prose + custom_count}),
    );
    report.inventory.insert("S22".into(), serde_json::json!({"has_agents_md": tracked.contains("AGENTS.md"), "budget_lines": 200, "docs": markdown}));
    Ok(())
}

fn instruction_facts(body: &str, tracked: &BTreeSet<&str>) -> serde_json::Value {
    let points = [
        "@AGENTS.md",
        "./AGENTS.md",
        "[AGENTS.md](AGENTS.md)",
        "[AGENTS.md](./AGENTS.md)",
    ];
    let links: BTreeSet<_> = body
        .split("](")
        .skip(1)
        .filter_map(|chunk| {
            chunk
                .split_once(')')
                .map(|(link, _)| link.trim_matches(['<', '>']))
        })
        .filter(|link| !link.is_empty() && !link.starts_with(['#', '/']) && !link.contains(':'))
        .filter(|link| {
            let path = PathBuf::from(link.split('#').next().unwrap_or_default());
            let canonical: PathBuf = path
                .components()
                .filter(|part| *part != Component::CurDir)
                .collect();
            !canonical.to_str().is_some_and(|path| {
                tracked.contains(path)
                    || tracked.iter().any(|name| {
                        name.strip_prefix(path)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                    })
            })
        })
        .collect();
    let lines = body.lines().count();
    let commands = [
        "cargo build",
        "cargo test",
        "cargo check",
        "npm run build",
        "npm test",
        "npm run lint",
        "just verify",
        "make check",
    ];
    serde_json::json!({
        "lines": lines,
        "over_budget": lines.saturating_sub(200),
        "points_to_agents": points.iter().any(|candidate| body.trim() == *candidate),
        "unresolved_local_links": links,
        "recognized_commands": commands.iter().filter(|command| body.contains(**command)).collect::<Vec<_>>(),
    })
}

fn read(repo: &Repo, path: &str) -> Result<Option<String>> {
    let file = repo.root.join(path);
    if !file
        .canonicalize()
        .is_ok_and(|canonical| canonical == file && canonical.is_file())
    {
        return Ok(None);
    }
    std::fs::read_to_string(file)
        .code("HYGIENE_INPUT")
        .map(Some)
}

fn workflow_checks(text: &str) -> Vec<&'static str> {
    [
        (
            "S1",
            &["cargo check", "tsc --noEmit", "npm run typecheck"][..],
        ),
        ("S4", &["cargo clippy", "eslint ", "npm run lint"][..]),
        (
            "S14",
            &[
                "cargo test",
                "cargo nextest",
                "npm test",
                "vitest ",
                "jest ",
            ][..],
        ),
        (
            "S23",
            &["cargo fmt", "prettier --check", "oxfmt --check"][..],
        ),
    ]
    .into_iter()
    .filter_map(|(id, markers)| {
        text.lines()
            .any(|line| {
                !line.trim().starts_with('#') && markers.iter().any(|marker| line.contains(marker))
            })
            .then_some(id)
    })
    .collect()
}

fn prose_rules(text: &str) -> usize {
    let mut fenced = false;
    text.lines()
        .filter(|line| {
            let line = line.trim();
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
            }
            if fenced || !(line.starts_with("- ") || line.starts_with("* ")) {
                return false;
            }
            line.split(|c: char| !c.is_alphanumeric() && c != '\'')
                .any(|word| {
                    ["always", "never", "don't", "must"]
                        .iter()
                        .any(|keyword| word.eq_ignore_ascii_case(keyword))
                })
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_instruction_proxies_and_local_links() {
        assert_eq!(
            prose_rules("- Always check.\n```\n- never count this\n```\n* Do not invent rules."),
            1
        );
        let tracked = BTreeSet::from(["README.md"]);
        let facts = instruction_facts(
            "[read](./README.md#work) [bad](missing.md) [web](https://example.com)\n`cargo test`",
            &tracked,
        );
        assert_eq!(
            facts["unresolved_local_links"],
            serde_json::json!(["missing.md"])
        );
        assert_eq!(
            facts["recognized_commands"],
            serde_json::json!(["cargo test"])
        );
        assert_eq!(
            workflow_checks("# cargo clippy\n      run: cargo test --locked"),
            ["S14"]
        );
    }
}
