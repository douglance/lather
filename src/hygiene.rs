mod architecture;
mod evidence;
mod inventory;
mod rust;
mod source;

use crate::model::{Error, Policy, Result, WithCode};
use crate::repo::{Profile, Repo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub weights: BTreeMap<String, u32>,
    pub function_lines: Option<u32>,
    pub rust_module_cycles: bool,
    pub boundaries: Vec<Boundary>,
    pub ui: Ui,
    pub evidence: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub from: String,
    pub forbid: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    pub forbidden_elements: BTreeSet<String>,
    pub forbidden_imports: BTreeSet<String>,
    pub component_module: Option<String>,
    pub forbidden_props: BTreeSet<String>,
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        let keys = [
            "S2", "S3", "S5", "S10", "S11", "S12", "S13", "S15", "S19", "S24", "S25",
        ];
        if self.function_lines == Some(0)
            || self
                .weights
                .iter()
                .any(|(key, weight)| !keys.contains(&key.as_str()) || !(1..=100).contains(weight))
            || self
                .boundaries
                .iter()
                .any(|rule| rule.from.is_empty() || rule.forbid.is_empty())
            || self.evidence.iter().any(|(key, args)| {
                !evidence::SUPPORTED.contains(&key.as_str())
                    || args.is_empty()
                    || args[0].is_empty()
                    || (key == "S14" && args.get(1).is_none_or(|arg| arg != "lather:landmines"))
            })
        {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Invalid hygiene weights, function limit, boundary, or evidence command",
            ));
        }
        Ok(())
    }

    pub fn weight(&self, signal: &str, default: u32) -> u32 {
        self.weights.get(signal).copied().unwrap_or(default)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct Observation {
    pub debt: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Site>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    pub path: String,
    pub line: usize,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct Report {
    pub signals: BTreeMap<String, Observation>,
    pub inventory: BTreeMap<String, serde_json::Value>,
    pub unavailable: BTreeMap<String, String>,
    pub scope: BTreeMap<String, String>,
    pub guards: BTreeMap<String, u64>,
    pub protections: BTreeSet<String>,
}

impl Report {
    pub(crate) fn signal(&mut self, id: &str) {
        self.signals.entry(id.into()).or_default();
    }

    pub(crate) fn record(
        &mut self,
        id: &str,
        path: &str,
        line: usize,
        detail: impl Into<String>,
        debt: u64,
    ) {
        let metric = self.signals.entry(id.into()).or_default();
        metric.debt += debt;
        if debt != 0 && metric.findings.len() < 256 {
            metric.findings.push(Site {
                path: path.into(),
                line,
                detail: detail.into(),
            });
        }
    }

    pub fn comparable(&self, other: &Self) -> bool {
        self.scope == other.scope && self.signals.keys().eq(other.signals.keys())
    }

    pub fn regressed(&self, before: &Self) -> bool {
        before.signals.iter().any(|(key, old)| {
            is_scored(key) && self.signals.get(key).is_none_or(|new| new.debt > old.debt)
        })
    }

    pub fn guards_regressed(&self, before: &Self) -> bool {
        !before.protections.is_subset(&self.protections)
            || self
                .guards
                .iter()
                .any(|(key, count)| *count > before.guards.get(key).copied().unwrap_or_default())
            || ["S15.lines", "S15.branches"].iter().any(|key| {
                let new = coverage_counts(self, key);
                let old = coverage_counts(before, key);
                old.is_some_and(|(covered, total)| {
                    new.is_none_or(|(next, next_total)| next * total < covered * next_total)
                })
            })
    }

    pub fn scored<'a>(
        &'a self,
        settings: &'a Settings,
    ) -> impl Iterator<Item = (&'a String, u32, &'a Observation)> {
        self.signals
            .iter()
            .filter(|(key, _)| is_scored(key))
            .map(|(key, value)| {
                let parent = key.split('.').next().unwrap_or(key);
                (key, settings.weight(parent, 5), value)
            })
    }
}

fn coverage_counts(report: &Report, key: &str) -> Option<(u128, u128)> {
    let counts = report.inventory.get(key)?;
    Some((
        counts["covered"].as_u64()?.into(),
        counts["total"].as_u64()?.into(),
    ))
}

fn is_scored(id: &str) -> bool {
    !matches!(id, "S8" | "S9" | "S14.external" | "S21")
}

pub fn observe(repo: &Repo, policy: &Policy, gate_ms: u64) -> Result<Report> {
    let mut report = Report::default();
    for id in ["S3", "S5", "S11", "S13", "S19", "S21", "S24", "S8", "S9"] {
        report.signal(id);
    }
    let paths = repo.git(&["ls-files", "-z", "--cached"])?;
    let tracked: BTreeSet<&str> = paths.split_terminator('\0').collect();
    let production = repo.sources()?.collect::<Result<BTreeMap<_, _>>>()?;
    let facts = match repo.profile {
        Profile::Rust => rust::collect(repo, &production, &policy.hygiene, &mut report),
        Profile::TypeScript => {
            source::typescript(repo, &production, &tracked, &policy.hygiene, &mut report)
        }
    }?;
    if repo.profile == Profile::TypeScript {
        report.signal("S25");
    }
    source::summarize(
        &facts,
        policy.hygiene.function_lines.unwrap_or(80),
        &mut report,
    );
    if policy.hygiene.ui != Ui::default() && repo.profile == Profile::TypeScript {
        report.signal("S10");
    }
    architecture::measure(&facts, &policy.hygiene.boundaries, &mut report);
    inventory::measure(repo, &tracked, policy, &mut report)?;
    report.inventory.insert("S16".into(), gate_ms.into());
    evidence::measure(repo, &policy.hygiene, &tracked, &mut report)?;
    Ok(report)
}

fn json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    serde_json::from_str(text).code("HYGIENE_OUTPUT")
}
