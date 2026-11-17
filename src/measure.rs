use crate::model::{Error, Gate, Result, Snapshot, Status, UnusedItem};
use crate::{
    process,
    repo::{Profile, Repo},
    signals, typescript,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

const CHECK_LIMIT: Duration = Duration::from_secs(240);
const RUST_CHECK: &[&str] = &[
    "check",
    "--offline",
    "--locked",
    "--workspace",
    "--all-targets",
    "--message-format=json",
];
const RUST_LINT: &[&str] = &[
    "clippy",
    "--offline",
    "--locked",
    "--workspace",
    "--all-targets",
    "--message-format=json",
];

pub fn scan(repo: &Repo, expected_commit: Option<&str>) -> Result<Snapshot> {
    let identity = repo.identity()?;
    if identity.dirty {
        return Err(Error::new(
            "WORKTREE_DIRTY",
            "Commit or remove worktree changes before measuring",
        ));
    }
    if expected_commit.is_some_and(|expected| identity.commit != expected) {
        return Err(Error::new(
            "STALE_CANDIDATE",
            "The registered worktree HEAD differs from the exact candidate commit",
        ));
    }

    let policy = repo.policy()?;
    let rust = repo.profile == Profile::Rust;
    let metrics = signals::measure(repo.sources()?, policy.file_lines, rust)?;
    let (
        environment,
        mut gates,
        unused_items,
        strict_flags,
        (test_gate, test_inventory, ignored_tests),
    ) = match repo.profile {
        Profile::Rust => {
            let (typecheck, unused) =
                diagnostic_gate(repo, "S1", RUST_CHECK, |level, _| level == "error");
            let (lint, tests) = if typecheck.status == Status::Fail {
                (
                    blocked_by_compiler("S4"),
                    (blocked_by_compiler("S14"), Vec::new(), 0),
                )
            } else {
                let (lint, _) = diagnostic_gate(repo, "S4", RUST_LINT, |level, code| {
                    level == "error" || (level == "warning" && code != Some("dead_code"))
                });
                (lint, test_gate(repo))
            };
            (
                environment(repo),
                vec![typecheck, lint, formatting_gate(repo)],
                Some(unused),
                None,
                tests,
            )
        }
        Profile::TypeScript => {
            let (gates, strict_flags) = typescript::gates(repo);
            (
                typescript::environment(repo),
                gates,
                None,
                strict_flags,
                typescript::tests(repo),
            )
        }
    };
    gates.push(test_gate);
    if repo.identity()? != identity {
        return Err(Error::new(
            "WORKTREE_CHANGED",
            "The worktree changed while checks were running",
        ));
    }
    Ok(Snapshot {
        repo: repo.id.clone(),
        worktree: repo.worktree.clone(),
        commit: identity.commit,
        environment,
        policy,
        gates,
        excess_lines: metrics
            .oversized
            .iter()
            .map(|finding| u64::from(finding.excess_lines))
            .sum(),
        findings: metrics.oversized,
        production_lines: metrics.lines,
        unchecked_exits: rust.then_some(metrics.unchecked_exits),
        unused_items,
        strict_flags,
        metric_issues: metrics.issues,
        test_inventory,
        ignored_tests,
    })
}

fn environment(repo: &Repo) -> String {
    let mut hash = Sha256::new();
    hash.update(b"rust-length-exits-unused-v5\0");
    for (program, args) in [
        ("cargo", &["-V"][..]),
        ("rustc", &["-vV"][..]),
        ("cargo", &["clippy", "-V"][..]),
        ("cargo", &["fmt", "--version"][..]),
    ] {
        let version = process::run(&repo.root, program, args, Duration::from_secs(15))
            .and_then(|output| output.checked(program));
        hash.update(match version {
            Ok(version) => version,
            Err(error) => format!("{}:{}", error.code, error.message),
        });
        hash.update([0]);
    }
    format!("{:x}", hash.finalize())
}

pub fn gate(id: &str, status: Status, debt: u64, detail: impl Into<String>) -> Gate {
    Gate {
        id: id.into(),
        status,
        debt,
        detail: detail.into(),
    }
}

pub fn unknown(id: &str, error: Error) -> Gate {
    gate(
        id,
        Status::Unknown,
        0,
        format!("{}: {}", error.code, error.message),
    )
}

fn blocked_by_compiler(id: &str) -> Gate {
    gate(
        id,
        Status::Blocked,
        0,
        "Requires a passing S1 compiler check",
    )
}

fn diagnostic_gate(
    repo: &Repo,
    id: &str,
    args: &[&str],
    counts: impl Fn(&str, Option<&str>) -> bool,
) -> (Gate, Vec<UnusedItem>) {
    let output = match process::run(&repo.root, "cargo", args, CHECK_LIMIT) {
        Ok(output) => output,
        Err(error) => return (unknown(id, error), Vec::new()),
    };
    let mut debt = 0;
    let mut finished = false;
    let mut unused = BTreeSet::new();
    for line in output.stdout.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match entry["reason"].as_str() {
            Some("compiler-message") => {
                let message = &entry["message"];
                let level = message["level"].as_str().unwrap_or_default();
                let code = message["code"]["code"].as_str();
                if counts(level, code) {
                    debt += 1;
                }
                if level == "warning"
                    && code == Some("dead_code")
                    && let Some(item) = unused_item(&repo.root, &entry, message)
                {
                    unused.insert(item);
                }
            }
            Some("build-finished") => finished = true,
            _ => {}
        }
    }
    let result = if debt > 0 {
        gate(id, Status::Fail, debt, output.diagnostic())
    } else if finished && output.status.success() {
        gate(id, Status::Pass, 0, "Completed")
    } else {
        gate(id, Status::Unknown, 0, output.diagnostic())
    };
    (result, unused.into_iter().collect())
}

fn unused_item(root: &Path, entry: &Value, message: &Value) -> Option<UnusedItem> {
    if !Path::new(entry["manifest_path"].as_str()?).starts_with(root) {
        return None;
    }
    let span = message["spans"]
        .as_array()?
        .iter()
        .find(|span| span["is_primary"].as_bool() == Some(true))?;
    let path = Path::new(span["file_name"].as_str()?);
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?
    } else {
        path
    };
    Some(UnusedItem {
        path: relative.to_str()?.into(),
        line: span["line_start"].as_u64()?,
        message: message["message"].as_str()?.into(),
    })
}

fn formatting_gate(repo: &Repo) -> Gate {
    let id = "S23";
    match process::run(
        &repo.root,
        "cargo",
        &["fmt", "--all", "--", "--check"],
        CHECK_LIMIT,
    ) {
        Ok(output) if output.status.success() => gate(id, Status::Pass, 0, "Completed"),
        Ok(output)
            if output.status.code() == Some(1)
                && output
                    .stdout
                    .lines()
                    .any(|line| line.starts_with("Diff in ")) =>
        {
            gate(id, Status::Fail, 1, output.diagnostic())
        }
        Ok(output) => gate(id, Status::Unknown, 0, output.diagnostic()),
        Err(error) => unknown(id, error),
    }
}

fn test_gate(repo: &Repo) -> (Gate, Vec<String>, u64) {
    let id = "S14";
    let output = match process::run(
        &repo.root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "--workspace",
            "--no-fail-fast",
        ],
        CHECK_LIMIT,
    ) {
        Ok(output) => output,
        Err(error) => return (unknown(id, error), Vec::new(), 0),
    };
    let mut inventory = BTreeSet::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut ignored = 0;
    let mut suites = 0;
    for line in output.stdout.lines() {
        if let Some((name, "ok" | "FAILED")) = line
            .strip_prefix("test ")
            .and_then(|test| test.rsplit_once(" ... "))
        {
            inventory.insert(name.to_string());
        }
        if let Some(summary) = line
            .strip_prefix("test result: ")
            .and_then(|line| line.split_once(". ").map(|(_, counts)| counts))
        {
            let mut fields = summary.split("; ");
            if let (Some(p), Some(f), Some(i)) = (
                fields.next().and_then(|field| read_count(field, " passed")),
                fields.next().and_then(|field| read_count(field, " failed")),
                fields
                    .next()
                    .and_then(|field| read_count(field, " ignored")),
            ) {
                passed += p;
                failed += f;
                ignored += i;
                suites += 1;
            }
        }
    }
    let result = if failed > 0 && output.status.code() == Some(101) {
        gate(id, Status::Fail, failed, output.diagnostic())
    } else if !output.status.success() || suites == 0 || (passed > 0 && inventory.is_empty()) {
        gate(id, Status::Unknown, 0, output.diagnostic())
    } else if passed == 0 {
        gate(id, Status::Fail, 1, "No tests ran")
    } else {
        gate(
            id,
            Status::Pass,
            0,
            format!("{passed} passed, {ignored} ignored"),
        )
    };
    (result, inventory.into_iter().collect(), ignored)
}

fn read_count(field: &str, suffix: &str) -> Option<u64> {
    field.strip_suffix(suffix)?.parse().ok()
}
