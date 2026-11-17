use super::{Store, repair_decision};
use crate::model::{Decision, Gate, Policy, Snapshot, Status, Verification};

fn snapshot(excess_lines: u64, lint_debt: u64, formatting_debt: u64) -> Snapshot {
    let gate = |id: &str, debt| Gate {
        id: id.into(),
        status: if debt == 0 {
            Status::Pass
        } else {
            Status::Fail
        },
        debt,
        detail: String::new(),
    };
    Snapshot {
        repo: "fixture".into(),
        worktree: "dev".into(),
        commit: "fixture-sha".into(),
        environment: "env".into(),
        policy: Policy::default(),
        gates: vec![
            gate("S1", 0),
            gate("S4", lint_debt),
            gate("S23", formatting_debt),
            gate("S14", 0),
        ],
        findings: Vec::new(),
        excess_lines,
        production_lines: 0,
        unchecked_exits: None,
        unused_items: None,
        strict_flags: None,
        metric_issues: Vec::new(),
        test_inventory: vec!["example".into()],
        ignored_tests: 0,
    }
}

#[test]
fn fixed_anchor_recognizes_improvement_only_when_all_gates_pass() {
    let anchor = snapshot(40, 0, 0);
    assert_eq!(anchor.score(&anchor), Some(0));
    assert_eq!(snapshot(30, 0, 0).score(&anchor), Some(2_500));
    assert_eq!(snapshot(20, 0, 0).score(&anchor), Some(5_000));
    assert_eq!(snapshot(20, 0, 1).score(&anchor), None);
    assert_eq!(snapshot(0, 0, 0).score(&snapshot(0, 0, 0)), Some(10_000));
}

#[test]
fn repairs_reduce_debt_without_breaking_another_gate() {
    let initial = snapshot(40, 2, 1);
    let repaired_lint = snapshot(40, 0, 1);
    let all_green = snapshot(40, 0, 0);
    assert_eq!(
        repair_decision(&initial, &repaired_lint).0,
        Decision::RepairProgress
    );
    assert_eq!(
        repair_decision(&repaired_lint, &all_green).0,
        Decision::RepairProgress
    );
    assert_eq!(
        repair_decision(&repaired_lint, &snapshot(40, 1, 0)).0,
        Decision::Rejected
    );
}

#[test]
fn compiler_repair_can_reveal_previously_blocked_failures() {
    let mut blocked = snapshot(40, 0, 0);
    blocked.gates[0].status = Status::Fail;
    blocked.gates[0].debt = 1;
    blocked.gates[1].status = Status::Blocked;
    blocked.gates[3].status = Status::Blocked;
    let with_lint_debt = snapshot(40, 2, 0);
    assert!(blocked.complete());
    assert!(!blocked.green());
    assert_eq!(
        repair_decision(&blocked, &with_lint_debt).0,
        Decision::RepairProgress
    );
    assert_eq!(
        repair_decision(&with_lint_debt, &blocked).0,
        Decision::Rejected
    );
}

#[test]
fn stale_verification_cannot_overwrite_an_advanced_tip() {
    let state = tempfile::tempdir().unwrap();
    let mut db = Store::open(state.path()).unwrap();
    let baseline = db.start("first", snapshot(40, 0, 0)).unwrap();
    let verdict = || Verification {
        baseline: baseline.id,
        before: baseline.tip,
        after: 0,
        decision: Decision::Accepted,
        reason: "fixture".into(),
        before_score_basis_points: Some(0),
        after_score_basis_points: Some(2500),
        diff_lines: 10,
    };
    let winner = db.record(verdict(), &snapshot(30, 0, 0)).unwrap();
    let stale = db.record(verdict(), &snapshot(20, 0, 0)).unwrap();
    assert_eq!(winner.decision, Decision::Accepted);
    assert_eq!(stale.decision, Decision::Inconclusive);
    let current = db.baseline(baseline.id).unwrap();
    assert_eq!(current.anchor, baseline.anchor);
    assert_eq!(current.tip, winner.after);
}
