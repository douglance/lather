use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub trait WithCode<T> {
    fn code(self, code: &'static str) -> Result<T>;
}

impl<T, E: std::fmt::Display> WithCode<T> for std::result::Result<T, E> {
    fn code(self, code: &'static str) -> Result<T> {
        self.map_err(|error| Error::new(code, error.to_string()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub file_lines: u32,
    pub max_diff_lines: u32,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            file_lines: 500,
            max_diff_lines: 100,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Blocked,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Gate {
    pub id: String,
    pub status: Status,
    pub debt: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Finding {
    pub path: String,
    pub lines: u32,
    pub excess_lines: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct UncheckedExit {
    pub path: String,
    pub line: usize,
    pub construct: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnusedItem {
    pub path: String,
    pub line: u64,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Snapshot {
    pub repo: String,
    pub worktree: String,
    pub commit: String,
    pub environment: String,
    pub policy: Policy,
    pub gates: Vec<Gate>,
    pub findings: Vec<Finding>,
    pub excess_lines: u64,
    #[serde(default)]
    pub production_lines: u64,
    #[serde(default)]
    pub unchecked_exits: Option<Vec<UncheckedExit>>,
    #[serde(default)]
    pub unused_items: Option<Vec<UnusedItem>>,
    #[serde(default)]
    pub strict_flags: Option<Vec<String>>,
    #[serde(default)]
    pub rust_unsafe_policy: Option<BTreeMap<String, bool>>,
    #[serde(default)]
    pub metric_issues: Vec<String>,
    pub test_inventory: Vec<String>,
    pub ignored_tests: u64,
}

#[derive(Default)]
pub struct Projection {
    pub fewer_lines: u64,
    pub fewer_exits: usize,
    pub fewer_unused: usize,
    pub more_strict_flags: usize,
    pub fewer_unprotected_rust_roots: usize,
}

impl Snapshot {
    pub fn green(&self) -> bool {
        self.gates.len() == 4 && self.gates.iter().all(|gate| gate.status == Status::Pass)
    }

    pub fn complete(&self) -> bool {
        self.gates.len() == 4 && self.gates.iter().all(|gate| gate.status != Status::Unknown)
    }

    pub fn score(&self, anchor: &Self) -> Option<u32> {
        if !self.green() || !anchor.green() || !self.comparable(anchor) {
            return None;
        }
        self.projected_score(anchor, Projection::default())
    }

    pub fn projected_score(&self, anchor: &Self, projection: Projection) -> Option<u32> {
        if !self.metric_issues.is_empty() || !anchor.metric_issues.is_empty() {
            return None;
        }
        let lines = normalized(
            self.excess_lines
                .saturating_sub(projection.fewer_lines)
                .into(),
            anchor.excess_lines.max(1).into(),
        );
        let mut weight = 5;
        let mut score = 5 * lines;
        match (&self.unchecked_exits, &anchor.unchecked_exits) {
            (None, None) => {}
            (Some(exits), Some(initial)) => {
                let denominator =
                    (initial.len() as u128 * 1_000).max(anchor.production_lines.max(1_000).into());
                let exits = normalized(
                    exits.len().saturating_sub(projection.fewer_exits) as u128 * 1_000,
                    denominator,
                );
                weight += 5;
                score += 5 * exits;
            }
            _ => return None,
        }
        match (&self.unused_items, &anchor.unused_items) {
            (None, None) => {}
            (Some(unused), Some(initial)) => {
                let unused = normalized(
                    unused.len().saturating_sub(projection.fewer_unused) as u128,
                    initial.len().max(1) as u128,
                );
                weight += 15;
                score += 15 * unused;
            }
            _ => return None,
        }
        match (&self.strict_flags, &anchor.strict_flags) {
            (None, None) => {}
            (Some(flags), Some(initial)) => {
                let total = crate::strictness::FLAGS.len();
                let debt =
                    total.saturating_sub(flags.len().saturating_add(projection.more_strict_flags));
                let denominator = total.saturating_sub(initial.len()).max(1);
                weight += 20;
                score += 20 * normalized(debt as u128, denominator as u128);
            }
            _ => return None,
        }
        match (&self.rust_unsafe_policy, &anchor.rust_unsafe_policy) {
            (Some(roots), Some(initial)) => {
                if !roots.is_empty() {
                    let debt = unprotected_roots(roots)
                        .saturating_sub(projection.fewer_unprotected_rust_roots);
                    let denominator = unprotected_roots(initial).max(1);
                    weight += 20;
                    score += 20 * normalized(debt as u128, denominator as u128);
                }
            }
            (None, None) => {}
            _ => return None,
        }
        Some(score / weight)
    }

    pub fn burden_increased(&self, before: &Self) -> bool {
        fn count<T>(items: &Option<Vec<T>>) -> usize {
            items.as_ref().map_or(0, Vec::len)
        }
        self.excess_lines > before.excess_lines
            || count(&self.unchecked_exits) > count(&before.unchecked_exits)
            || count(&self.unused_items) > count(&before.unused_items)
            || before
                .strict_flags
                .as_ref()
                .zip(self.strict_flags.as_ref())
                .is_some_and(|(old, new)| old.iter().any(|flag| !new.contains(flag)))
    }

    pub fn rust_policy_regressed(&self, before: &Self) -> bool {
        before
            .rust_unsafe_policy
            .as_ref()
            .zip(self.rust_unsafe_policy.as_ref())
            .is_some_and(|(old, new)| {
                old.iter()
                    .any(|(path, enforced)| *enforced && new.get(path) != Some(&true))
            })
    }

    pub fn comparable(&self, other: &Self) -> bool {
        self.repo == other.repo
            && self.environment == other.environment
            && self.policy == other.policy
            && self
                .rust_unsafe_policy
                .as_ref()
                .zip(other.rust_unsafe_policy.as_ref())
                .is_none_or(|(left, right)| left.keys().eq(right.keys()))
    }
}

fn unprotected_roots(roots: &BTreeMap<String, bool>) -> usize {
    roots.values().filter(|enforced| !**enforced).count()
}

fn normalized(debt: u128, denominator: u128) -> u32 {
    (10_000 * denominator.saturating_sub(debt) / denominator) as u32
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct StoredSnapshot {
    pub id: i64,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Baseline {
    pub id: i64,
    pub name: String,
    pub anchor: i64,
    pub tip: i64,
    pub repair_mode: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Candidate {
    pub based_on: i64,
    pub kind: String,
    pub subject: String,
    pub detail: String,
    pub max_diff_lines: u32,
    pub expected_gain_basis_points: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accepted,
    Rejected,
    Inconclusive,
    RepairProgress,
}

impl Decision {
    pub fn advances(self) -> bool {
        matches!(self, Self::Accepted | Self::RepairProgress)
    }

    pub fn exit_code(self) -> i32 {
        match self {
            Self::Accepted => 0,
            Self::Rejected => 2,
            Self::Inconclusive => 3,
            Self::RepairProgress => 4,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Verification {
    pub baseline: i64,
    pub before: i64,
    pub after: i64,
    pub decision: Decision,
    pub reason: String,
    pub before_score_basis_points: Option<u32>,
    pub after_score_basis_points: Option<u32>,
    pub diff_lines: u64,
}
