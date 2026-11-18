mod formats;

use super::{Report, Settings, Site, json};
use crate::model::{Error, Result, WithCode};
use crate::process;
use crate::repo::Repo;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

pub const SUPPORTED: &[&str] = &["S12", "S13", "S14", "S15", "S24", "S25"];

pub fn measure(
    repo: &Repo,
    policy: &Settings,
    tracked: &BTreeSet<&str>,
    report: &mut Report,
) -> Result<()> {
    for &id in SUPPORTED {
        let Some(args) = policy.evidence.get(id) else {
            report.unavailable.insert(
                format!("{id}.external"),
                "No evidence command configured".into(),
            );
            continue;
        };
        let command = &args[0];
        let file = if Path::new(command).is_absolute() {
            Path::new(command).to_path_buf()
        } else if command.starts_with("hygiene-checks/") && tracked.contains(command.as_str()) {
            repo.root.join(command)
        } else {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Evidence commands must use an absolute executable or a tracked hygiene-checks/ executable",
            ));
        };
        let executable = file.canonicalize().code("EVIDENCE_TOOL")?;
        if executable.starts_with(&repo.root)
            && (executable != file || Path::new(command).is_absolute())
        {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Worktree evidence executables must use tracked hygiene-checks/ paths without symlinks",
            ));
        }
        let mut hasher = Sha256::new();
        let mut bytes = std::fs::File::open(&executable).code("EVIDENCE_TOOL")?;
        std::io::copy(&mut bytes, &mut hasher).code("EVIDENCE_TOOL")?;
        let mut format = None;
        let start = if args.get(1).is_some_and(|arg| arg.starts_with("lather:")) {
            format = args[1].strip_prefix("lather:");
            2
        } else {
            1
        };
        let argv: Vec<_> = args[start..].iter().map(String::as_str).collect();
        hash_arguments(repo, &argv, &mut hasher)?;
        let absolute = executable
            .to_str()
            .ok_or_else(|| Error::new("EVIDENCE_TOOL", "Evidence executable path is not UTF-8"))?;
        let output = process::run(&repo.root, absolute, &argv, Duration::from_secs(300))?;
        if !output.status.success()
            && !(matches!(format, Some("eslint" | "knip" | "landmines" | "machete"))
                && output.status.code() == Some(1))
        {
            return Err(Error::new(
                "EVIDENCE_FAILED",
                format!("{id}: {}", output.diagnostic()),
            ));
        }
        let measured = if id == "S14" {
            formats::landmines(&output.stdout, !output.status.success())?
        } else {
            formats::decode(id, format, &output.stdout)?
        };
        report
            .scope
            .insert(format!("{id}.external"), format!("{:x}", hasher.finalize()));
        store(id, repo, tracked, measured, report)?;
    }
    Ok(())
}

fn hash_arguments(repo: &Repo, argv: &[&str], hasher: &mut Sha256) -> Result<()> {
    for argument in argv {
        let value = argument
            .split_once('=')
            .map_or(*argument, |(_, value)| value);
        let path = repo.root.join(value);
        if path.is_file() {
            let resolved = path.canonicalize().code("EVIDENCE_TOOL")?;
            if let Ok(relative) = resolved.strip_prefix(&repo.root)
                && !relative.to_str().is_some_and(crate::integrity::policy_path)
            {
                return Err(Error::new(
                    "CONFIG_INVALID",
                    format!("Evidence argument references an unprotected input: {value}"),
                ));
            }
            let mut bytes = std::fs::File::open(resolved).code("EVIDENCE_TOOL")?;
            hasher.update([0]);
            std::io::copy(&mut bytes, &mut *hasher).code("EVIDENCE_TOOL")?;
        }
    }
    Ok(())
}

fn store(
    id: &str,
    repo: &Repo,
    tracked: &BTreeSet<&str>,
    evidence: Evidence,
    report: &mut Report,
) -> Result<()> {
    if id == "S15" {
        let lines = evidence
            .lines
            .ok_or_else(|| Error::new("EVIDENCE_OUTPUT", "Coverage requires line counts"))?;
        if lines.total == 0 || !evidence.findings.is_empty() {
            return Err(Error::new(
                "EVIDENCE_OUTPUT",
                "Expected nonempty coverage counts and no findings for S15",
            ));
        }
        coverage("S15.lines", lines, report)?;
        if let Some(branches) = evidence.branches.filter(|branches| branches.total != 0) {
            coverage("S15.branches", branches, report)?;
        }
    } else {
        if evidence.lines.is_some() || evidence.branches.is_some() {
            return Err(Error::new(
                "EVIDENCE_OUTPUT",
                "Coverage counts are only valid for S15",
            ));
        }
        let key = format!("{id}.external");
        report.signal(&key);
        let mut seen = BTreeSet::new();
        let findings = evidence.findings.into_iter().map(|site| (None, site));
        let test_integrity = evidence
            .test_integrity
            .into_iter()
            .map(|(rule, site)| (Some(rule), site));
        for (rule, site) in findings.chain(test_integrity) {
            let path = Path::new(&site.path);
            let relative = if path.is_absolute() {
                path.strip_prefix(&repo.root).ok().and_then(Path::to_str)
            } else {
                Some(site.path.trim_start_matches("./"))
            };
            let Some(path) = relative.filter(|&path| tracked.contains(path)) else {
                return Err(Error::new(
                    "EVIDENCE_SCOPE",
                    format!("{id} reported an untracked path: {}", site.path),
                ));
            };
            if site.line == 0 || site.detail.trim().is_empty() {
                return Err(Error::new(
                    "EVIDENCE_OUTPUT",
                    "A finding requires a line and a description",
                ));
            }
            if let Some(rule) = rule {
                *report
                    .guards
                    .entry(format!("S14 {rule} {path}"))
                    .or_default() += 1;
            } else if !seen.insert((path.to_owned(), site.line, site.detail.clone())) {
                continue;
            }
            report.record(&key, path, site.line, site.detail, 1);
        }
    }
    Ok(())
}

fn coverage(key: &str, counts: Counts, report: &mut Report) -> Result<()> {
    let Some(uncovered) = counts.total.checked_sub(counts.covered) else {
        return Err(Error::new(
            "EVIDENCE_OUTPUT",
            "Covered units exceed the total",
        ));
    };
    report.signal(key);
    report
        .signals
        .get_mut(key)
        .ok_or_else(|| Error::new("EVIDENCE_OUTPUT", "Coverage signal is missing"))?
        .debt = uncovered;
    report.inventory.insert(
        key.into(),
        serde_json::json!({"covered": counts.covered, "total": counts.total}),
    );
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Evidence {
    #[serde(default)]
    findings: Vec<Site>,
    #[serde(skip)]
    test_integrity: Vec<(String, Site)>,
    lines: Option<Counts>,
    branches: Option<Counts>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    covered: u64,
    total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::Profile;

    #[test]
    fn landmines_guards_each_file_and_rule_without_scoring() {
        let repo = Repo {
            id: "example".into(),
            worktree: "dev".into(),
            root: "/worktree".into(),
            profile: Profile::TypeScript,
        };
        let tracked = BTreeSet::from(["a.test.js", "b.test.js"]);
        let report = |findings: &[(&str, &str)]| {
            let diagnostics: Vec<_> = findings
                .iter()
                .map(|(path, rule)| {
                    serde_json::json!({
                        "code": format!("landmines-testing({rule})"),
                        "severity": "error",
                        "filename": path,
                        "labels": [{"span": {"line": 4}}],
                        "message": "Await the operation"
                    })
                })
                .collect();
            let output = serde_json::json!({"diagnostics": diagnostics, "number_of_files": 2});
            let evidence = formats::landmines(&output.to_string(), !findings.is_empty()).unwrap();
            let mut report = Report::default();
            store("S14", &repo, &tracked, evidence, &mut report).unwrap();
            report
        };
        let sleep = ("a.test.js", "no-fixed-sleep");
        let before = report(&[sleep, sleep]);
        assert_eq!(before.signals["S14.external"].debt, 2);
        assert!(!report(&[sleep]).guards_regressed(&before));
        assert!(report(&[sleep, sleep, sleep]).guards_regressed(&before));
        assert!(report(&[sleep, ("b.test.js", sleep.1)]).guards_regressed(&before));
        assert!(report(&[sleep, (sleep.0, "no-flake-masking")]).guards_regressed(&before));
        assert_eq!(before.scored(&Settings::default()).count(), 0);
    }
}
