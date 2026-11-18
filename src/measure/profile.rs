use super::{
    RUST_CHECK, RUST_LINT, blocked_by_compiler, diagnostic_gate, environment, formatting_gate,
    test_gate,
};
use crate::model::{Gate, Status, UnusedItem};
use crate::repo::{Profile, Repo};
use crate::typescript;

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
