use crate::measure::{gate, unknown};
use crate::model::{Error, Gate, Result, Status, WithCode};
use crate::process::{self, Output};
use crate::repo::Repo;
use crate::strictness;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(240);
const TSC: &str = "node_modules/typescript/bin/tsc";
const ESLINT: &str = "node_modules/eslint/bin/eslint.js";
const PRETTIER: &str = "node_modules/prettier/bin/prettier.cjs";

pub fn environment(repo: &Repo) -> String {
    let mut hash = Sha256::new();
    hash.update(b"typescript-hygiene-catalog-v3\0");
    for args in [
        &["--version"][..],
        &[TSC, "--version"][..],
        &[ESLINT, "--version"][..],
        &[PRETTIER, "--version"][..],
    ] {
        let version = process::run(&repo.root, "node", args, Duration::from_secs(15))
            .and_then(|output| output.checked("node"));
        hash.update(match version {
            Ok(version) => version,
            Err(error) => error.code.into(),
        });
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())
}

pub fn gates(repo: &Repo) -> (Vec<Gate>, Option<Vec<String>>) {
    let (types, strict_flags) =
        typecheck(repo).unwrap_or_else(|error| (unknown("S1", error), None));
    let lint = lint(repo).unwrap_or_else(|error| unknown("S4", error));
    let format = formatting(repo).unwrap_or_else(|error| unknown("S23", error));
    (vec![types, lint, format], strict_flags)
}

fn node_tool(repo: &Repo, program: &str, args: &[&str]) -> Result<Output> {
    if !repo.root.join(program).is_file() {
        return Err(Error::new(
            "TOOL_MISSING",
            format!("Run npm ci in the trusted worktree to install {program}"),
        ));
    }
    let args: Vec<_> = std::iter::once(program)
        .chain(args.iter().copied())
        .collect();
    process::run(&repo.root, "node", &args, LIMIT)
}

fn typecheck(repo: &Repo) -> Result<(Gate, Option<Vec<String>>)> {
    let id = "S1";
    let output = node_tool(
        repo,
        TSC,
        &[
            "--noEmit",
            "--pretty",
            "false",
            "--project",
            "tsconfig.json",
        ],
    )?;
    if !output.status.success() {
        let debt = output
            .stdout
            .lines()
            .filter(|line| {
                line.split_once("error TS").is_some_and(|(_, suffix)| {
                    suffix.split_once(':').is_some_and(|(code, _)| {
                        !code.is_empty() && code.bytes().all(|byte| byte.is_ascii_digit())
                    })
                })
            })
            .count() as u64;
        let status = if debt > 0 {
            Status::Fail
        } else {
            Status::Unknown
        };
        return Ok((gate(id, status, debt, output.diagnostic()), None));
    }
    let config = node_tool(repo, TSC, &["--showConfig", "--project", "tsconfig.json"])?
        .checked("tsc --showConfig")?;
    let config: Value = serde_json::from_str(&config).code("OUTPUT_UNPARSEABLE")?;
    let files = config["files"].as_array().ok_or_else(|| {
        Error::new(
            "CHECK_SCOPE_UNKNOWN",
            "No source files in effective tsconfig",
        )
    })?;
    let has_typescript = files.iter().any(|file| {
        file.as_str().is_some_and(|file| {
            !file.ends_with(".d.ts")
                && [".ts", ".tsx", ".cts", ".mts"]
                    .iter()
                    .any(|extension| file.ends_with(extension))
        })
    });
    if !has_typescript && config["compilerOptions"]["checkJs"].as_bool() != Some(true) {
        return Err(Error::new(
            "CHECK_SCOPE_UNKNOWN",
            "A JavaScript profile requires checkJs in the effective tsconfig",
        ));
    }
    if config["compilerOptions"]["noCheck"].as_bool() == Some(true) {
        return Err(Error::new(
            "CHECK_SCOPE_UNKNOWN",
            "The effective tsconfig disables type checking",
        ));
    }
    let evidence = gate(
        id,
        Status::Pass,
        0,
        format!("{} files in effective tsconfig", files.len()),
    );
    Ok((
        evidence,
        Some(strictness::enabled(&config["compilerOptions"])),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LintFile {
    error_count: u64,
    warning_count: u64,
}

fn lint(repo: &Repo) -> Result<Gate> {
    let id = "S4";
    let output = node_tool(repo, ESLINT, &[".", "--format", "json"])?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        return Ok(gate(id, Status::Unknown, 0, output.diagnostic()));
    }
    let files: Vec<LintFile> = serde_json::from_str(&output.stdout).code("OUTPUT_UNPARSEABLE")?;
    let debt = files
        .iter()
        .map(|file| file.error_count + file.warning_count)
        .sum::<u64>();
    if debt > 0 {
        Ok(gate(
            id,
            Status::Fail,
            debt,
            format!("{debt} ESLint errors or warnings"),
        ))
    } else if files.is_empty() || !output.status.success() {
        Ok(gate(
            id,
            Status::Unknown,
            0,
            "ESLint returned no usable file evidence",
        ))
    } else {
        Ok(gate(
            id,
            Status::Pass,
            0,
            format!("{} files linted", files.len()),
        ))
    }
}

fn formatting(repo: &Repo) -> Result<Gate> {
    let id = "S23";
    let output = node_tool(repo, PRETTIER, &[".", "--check"])?;
    if output.status.success()
        && output
            .stdout
            .contains("All matched files use Prettier code style!")
    {
        Ok(gate(id, Status::Pass, 0, "Completed"))
    } else if output.status.code() == Some(1)
        && output.stderr.contains("Code style issues found in")
    {
        Ok(gate(id, Status::Fail, 1, output.diagnostic()))
    } else {
        Ok(gate(id, Status::Unknown, 0, output.diagnostic()))
    }
}

const TEST_GATE: &str = "S14";

pub fn tests(repo: &Repo) -> (Gate, Vec<String>, u64) {
    run_tests(repo).unwrap_or_else(|error| (unknown(TEST_GATE, error), Vec::new(), 0))
}

#[derive(Deserialize)]
struct TestCounts {
    passed: u64,
    failed: u64,
    cancelled: u64,
    skipped: u64,
    todo: u64,
}

fn run_tests(repo: &Repo) -> Result<(Gate, Vec<String>, u64)> {
    let files = repo.node_tests()?;
    if files.is_empty() {
        return Ok((
            gate(
                TEST_GATE,
                Status::Fail,
                1,
                "No tracked Node test entrypoints found",
            ),
            Vec::new(),
            0,
        ));
    }
    let version = process::run(&repo.root, "node", &["--version"], Duration::from_secs(15))?
        .checked("node")?;
    if !version.starts_with("v24.") {
        return Err(Error::new(
            "TOOL_VERSION_UNSUPPORTED",
            "The structured Node test adapter requires Node 24",
        ));
    }
    let mut reporter = tempfile::Builder::new()
        .suffix(".mjs")
        .tempfile()
        .code("RUNNER_IO")?;
    reporter
        .write_all(include_bytes!("node-test.mjs"))
        .code("RUNNER_IO")?;
    let reporter_path = reporter
        .path()
        .to_str()
        .ok_or_else(|| Error::new("RUNNER_IO", "Test reporter path must be UTF-8"))?;
    let args: Vec<_> = std::iter::once(reporter_path)
        .chain(files.iter().map(String::as_str))
        .collect();
    let output = process::run(&repo.root, "node", &args, LIMIT)?;
    let mut counts = None;
    let mut tests = BTreeSet::new();
    for line in output.stdout.lines() {
        let entry: Value = serde_json::from_str(line).code("OUTPUT_UNPARSEABLE")?;
        if let Some(raw_counts) = entry.get("counts") {
            counts = Some(
                serde_json::from_value::<TestCounts>(raw_counts.clone())
                    .code("OUTPUT_UNPARSEABLE")?,
            );
        } else if let (Some(file), Some(name)) = (entry["file"].as_str(), entry["name"].as_str()) {
            let relative = Path::new(file)
                .strip_prefix(&repo.root)
                .code("OUTPUT_UNPARSEABLE")?;
            tests.insert(format!("{}::{name}", relative.display()));
        }
    }
    let counts = counts.ok_or_else(|| {
        Error::new(
            "OUTPUT_UNPARSEABLE",
            "Node test run did not produce aggregate counts",
        )
    })?;
    let debt = counts.failed + counts.cancelled;
    let result = if debt > 0 && output.status.code() == Some(1) {
        gate(
            TEST_GATE,
            Status::Fail,
            debt,
            format!(
                "{} tests failed, {} cancelled",
                counts.failed, counts.cancelled
            ),
        )
    } else if !output.status.success() || (counts.passed > 0 && tests.is_empty()) {
        gate(TEST_GATE, Status::Unknown, 0, output.diagnostic())
    } else if counts.passed == 0 {
        gate(TEST_GATE, Status::Fail, 1, "No tests ran")
    } else {
        gate(
            TEST_GATE,
            Status::Pass,
            0,
            format!("{} tests passed", counts.passed),
        )
    };
    Ok((
        result,
        tests.into_iter().collect(),
        counts.skipped + counts.todo,
    ))
}
