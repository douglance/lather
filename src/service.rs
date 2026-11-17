use crate::model::{
    Baseline, Candidate, Decision, Error, Result, Snapshot, Status, StoredSnapshot, Verification,
    WithCode,
};
use crate::repo::{Repo, valid_id};
use crate::{
    measure, propose,
    store::{DATABASE_FILE, LEGACY_DATABASE_FILE, Store},
};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

pub struct Service {
    pub store: Store,
    registry: PathBuf,
}

#[derive(Serialize, JsonSchema)]
pub struct Score {
    pub snapshot: i64,
    pub baseline: i64,
    pub commit: String,
    pub score_basis_points: Option<u32>,
    pub excess_lines: u64,
    pub comparable: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Proposal {
    pub tip: i64,
    pub mode: Mode,
    pub candidate: Option<Candidate>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Scrub,
    Repair,
    ReadyForBaseline,
    Exhausted,
}

#[derive(Serialize, JsonSchema)]
pub struct Trend {
    pub repo: String,
    pub baseline: Baseline,
    pub anchor_commit: String,
    pub tip_commit: String,
    pub score_basis_points: Option<u32>,
}

const WASHLIST_FILE: &str = "washlist.toml";
const LEGACY_WASHLIST_FILE: &str = "repos.toml";

impl Service {
    pub fn open() -> Result<Self> {
        let state = match path_override(&["LATHER_HOME", "HYGIENE_CLIMB_HOME"]) {
            Some(path) => path,
            None => data_directory(
                "XDG_STATE_HOME",
                ".local/state",
                &[DATABASE_FILE, LEGACY_DATABASE_FILE],
            )?,
        };
        let registry = match path_override(&[
            "LATHER_WASHLIST",
            "LATHER_REGISTRY",
            "HYGIENE_CLIMB_REGISTRY",
        ]) {
            Some(path) => path,
            None => {
                let directory = data_directory(
                    "XDG_CONFIG_HOME",
                    ".config",
                    &[WASHLIST_FILE, LEGACY_WASHLIST_FILE],
                )?;
                let washlist = directory.join(WASHLIST_FILE);
                let legacy = directory.join(LEGACY_WASHLIST_FILE);
                if !washlist.try_exists().code("PATH_IO")? && legacy.try_exists().code("PATH_IO")? {
                    legacy
                } else {
                    washlist
                }
            }
        };
        Ok(Self {
            store: Store::open(&state)?,
            registry,
        })
    }

    fn repo(&self, repo: &str, worktree: &str) -> Result<Repo> {
        Repo::resolve(&self.registry, repo, worktree)
    }

    pub fn scan(&self, repo: &str, worktree: &str) -> Result<StoredSnapshot> {
        self.store
            .save(measure::scan(&self.repo(repo, worktree)?, None)?)
    }

    pub fn baseline(&mut self, repo: &str, worktree: &str, name: &str) -> Result<Baseline> {
        if !valid_id(name) {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Baseline name must contain 1–64 ASCII letters, digits, hyphens, or underscores and start with a letter or digit",
            ));
        }
        let snapshot = measure::scan(&self.repo(repo, worktree)?, None)?;
        self.store.start(name, snapshot)
    }

    pub fn score(&self, id: i64, baseline: i64) -> Result<Score> {
        let snapshot = self.store.snapshot(id)?.snapshot;
        let anchor = self
            .store
            .snapshot(self.store.baseline(baseline)?.anchor)?
            .snapshot;
        Ok(Score {
            snapshot: id,
            baseline,
            score_basis_points: snapshot.score(&anchor),
            excess_lines: snapshot.excess_lines,
            comparable: snapshot.comparable(&anchor),
            commit: snapshot.commit,
        })
    }

    pub fn trend(&self, id: i64) -> Result<Trend> {
        let baseline = self.store.baseline(id)?;
        let anchor = self.store.snapshot(baseline.anchor)?.snapshot;
        let tip = self.store.snapshot(baseline.tip)?.snapshot;
        Ok(Trend {
            repo: tip.repo.clone(),
            score_basis_points: tip.score(&anchor),
            baseline,
            anchor_commit: anchor.commit,
            tip_commit: tip.commit,
        })
    }

    fn compare(&self, repo: &str, baseline: i64) -> Result<(Baseline, Snapshot, Snapshot)> {
        let baseline = self.store.baseline(baseline)?;
        let anchor = self.store.snapshot(baseline.anchor)?.snapshot;
        let tip = self.store.snapshot(baseline.tip)?.snapshot;
        if anchor.repo != repo || tip.repo != repo {
            return Err(Error::new(
                "BASELINE_REPO_MISMATCH",
                "This baseline belongs to a different repository",
            ));
        }
        Ok((baseline, anchor, tip))
    }

    pub fn next(&self, repo: &str, worktree: &str, id: i64) -> Result<Proposal> {
        let (baseline, anchor, tip) = self.compare(repo, id)?;
        let repo = self.repo(repo, worktree)?;
        let identity = repo.identity()?;
        if identity.dirty || identity.commit != tip.commit {
            return Err(Error::new(
                "STALE_CANDIDATE",
                "Use a clean worktree at the current baseline tip",
            ));
        }
        let max_diff_lines = tip.policy.max_diff_lines;
        let (mode, candidate) = if baseline.repair_mode {
            match tip.gates.iter().find(|gate| gate.status == Status::Fail) {
                Some(gate) => (
                    Mode::Repair,
                    Some(Candidate {
                        based_on: baseline.tip,
                        kind: "repair_gate".into(),
                        subject: gate.id.clone(),
                        detail: gate.detail.clone(),
                        max_diff_lines,
                        expected_gain_basis_points: None,
                    }),
                ),
                None => (Mode::ReadyForBaseline, None),
            }
        } else if let Some(candidate) = propose::highest_gain(baseline.tip, &tip, &anchor) {
            (Mode::Scrub, Some(candidate))
        } else {
            (Mode::Exhausted, None)
        };
        Ok(Proposal {
            tip: baseline.tip,
            mode,
            candidate,
        })
    }

    pub fn verify(
        &mut self,
        repo: &str,
        worktree: &str,
        id: i64,
        against: i64,
        commit: &str,
    ) -> Result<Verification> {
        let (baseline, anchor, before) = self.compare(repo, id)?;
        if against != baseline.tip {
            return Err(Error::new(
                "STALE_CANDIDATE",
                "Comparison must use the current baseline tip",
            ));
        }
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(Error::new(
                "COMMIT_INVALID",
                "Provide the full candidate commit SHA",
            ));
        }
        let repo = self.repo(repo, worktree)?;
        let after = measure::scan(&repo, Some(commit))?;
        let diff = repo.diff(&before.commit, commit)?;
        let protected: Vec<_> = diff.protected.into_iter().collect();
        let before_score = before.score(&anchor);
        let after_score = after.score(&anchor);
        let (decision, reason) = if !after.comparable(&anchor)
            || !after.complete()
            || (!baseline.repair_mode && !after.metric_issues.is_empty())
        {
            (
                Decision::Inconclusive,
                "Policy, environment, or available check evidence changed".into(),
            )
        } else if !protected.is_empty() {
            (
                Decision::Rejected,
                format!("Protected inputs changed: {}", protected.join(", ")),
            )
        } else if diff.lines > u64::from(before.policy.max_diff_lines) {
            (
                Decision::Rejected,
                format!(
                    "Diff exceeds {} changed lines",
                    before.policy.max_diff_lines
                ),
            )
        } else if test_inventory_regressed(&before, &after) {
            (
                Decision::Rejected,
                "The number of ignored tests increased or a previously executed test disappeared"
                    .into(),
            )
        } else if baseline.repair_mode {
            repair_decision(&before, &after)
        } else if !after.green() {
            (Decision::Rejected, "A hard gate failed".into())
        } else if after.burden_increased(&before) {
            (Decision::Rejected, "A scored signal regressed".into())
        } else if matches!((before_score, after_score), (Some(before), Some(after)) if after > before)
        {
            (
                Decision::Accepted,
                "Score improved and all hard gates passed".into(),
            )
        } else {
            (Decision::Rejected, "Score did not improve".into())
        };
        self.store.record(
            Verification {
                baseline: id,
                before: against,
                after: 0,
                decision,
                reason,
                before_score_basis_points: before_score,
                after_score_basis_points: after_score,
                diff_lines: diff.lines,
            },
            &after,
        )
    }
}

fn path_override(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(std::env::var_os).map(PathBuf::from)
}

fn data_directory(xdg_key: &str, fallback: &str, filenames: &[&str]) -> Result<PathBuf> {
    let parent = match std::env::var_os(xdg_key) {
        Some(parent) => PathBuf::from(parent),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(fallback))
            .ok_or_else(|| {
                Error::new(
                    "CONFIG_INVALID",
                    "HOME is required when XDG paths are unset",
                )
            })?,
    };
    let lather = parent.join("lather");
    let legacy = parent.join("hygiene-climb");
    for filename in filenames {
        if lather.join(filename).try_exists().code("PATH_IO")? {
            return Ok(lather);
        }
    }
    for filename in filenames {
        if legacy.join(filename).try_exists().code("PATH_IO")? {
            return Ok(legacy);
        }
    }
    Ok(lather)
}

fn test_inventory_regressed(before: &Snapshot, after: &Snapshot) -> bool {
    let tests_ran = before
        .gates
        .iter()
        .any(|gate| gate.id == "S14" && gate.status != Status::Blocked);
    tests_ran
        && (after.ignored_tests > before.ignored_tests
            || before
                .test_inventory
                .iter()
                .any(|test| after.test_inventory.binary_search(test).is_err()))
}

fn repair_decision(before: &Snapshot, after: &Snapshot) -> (Decision, String) {
    let pairs = before.gates.iter().zip(&after.gates);
    if pairs.clone().any(|(old, new)| {
        old.id != new.id
            || (old.status != Status::Blocked
                && (new.status == Status::Blocked || new.debt > old.debt))
    }) {
        (Decision::Rejected, "Hard-gate debt increased".into())
    } else if pairs.clone().any(|(old, new)| {
        old.status == Status::Fail && new.status != Status::Blocked && new.debt < old.debt
    }) {
        let reason = if after.green() {
            "All gates pass. Run soak to start a scoring baseline"
        } else {
            "Hard-gate debt decreased"
        };
        (Decision::RepairProgress, reason.into())
    } else {
        (Decision::Rejected, "Hard-gate debt did not decrease".into())
    }
}

#[cfg(test)]
#[path = "tests/verification.rs"]
mod tests;
