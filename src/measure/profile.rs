use super::{
    RUST_CHECK, RUST_LINT, blocked_by_compiler, diagnostic_gate, environment, formatting_gate,
    gate, test_gate,
};
use crate::model::{Gate, Status, UnusedItem};
use crate::repo::{Profile, Repo};
use crate::{hygiene, typescript};

pub(super) struct Checks {
    pub(super) environment: String,
    pub(super) gates: Vec<Gate>,
    pub(super) unused_items: Option<Vec<UnusedItem>>,
    pub(super) strict_flags: Option<Vec<String>>,
    pub(super) tests: (Gate, Vec<String>, u64),
}

pub(super) fn run(repo: &Repo) -> Checks {
    let (environment, gates, unused_items, strict_flags, tests) = match repo.profile {
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
    Checks {
        environment,
        gates,
        unused_items,
        strict_flags,
        tests,
    }
}

pub(super) fn add_graph_gates(gates: &mut Vec<Gate>, report: &hygiene::Report) {
    let compiler_failed = gates
        .iter()
        .any(|gate| gate.id == "S1" && gate.status == Status::Fail);
    for id in ["S8", "S9"] {
        let observation = &report.signals[id];
        let result = if let Some(reason) = report.unavailable.get("source-analysis") {
            if compiler_failed {
                blocked_by_compiler(id)
            } else {
                gate(id, Status::Unknown, 0, reason)
            }
        } else if observation.debt > 0 {
            gate(
                id,
                Status::Fail,
                observation.debt,
                &observation.findings[0].detail,
            )
        } else {
            gate(id, Status::Pass, 0, "No violations in the measured graph")
        };
        gates.push(result);
    }
}
