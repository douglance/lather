use crate::model::{Candidate, Projection, Snapshot};
use crate::strictness;

pub fn highest_gain(id: i64, tip: &Snapshot, anchor: &Snapshot) -> Option<Candidate> {
    let current = tip.score(anchor)?;
    let max_diff = tip.policy.max_diff_lines;
    let mut options = Vec::new();

    for finding in &tip.findings {
        let lines = finding.excess_lines.min(max_diff / 2);
        let cost = lines * 2;
        let gain = tip
            .projected_score(
                anchor,
                Projection {
                    fewer_lines: lines.into(),
                    ..Projection::default()
                },
            )?
            .saturating_sub(current);
        options.push((
            gain,
            cost,
            "reduce_oversized_file",
            finding.path.clone(),
            format!(
                "Reduce this file from {} toward {} lines with one cohesive change",
                finding.lines, tip.policy.file_lines,
            ),
        ));
    }
    if let Some(exit) = tip
        .unchecked_exits
        .as_ref()
        .and_then(|exits| exits.first())
        .filter(|_| max_diff >= 6)
    {
        let gain = tip
            .projected_score(
                anchor,
                Projection {
                    fewer_exits: 1,
                    ..Projection::default()
                },
            )?
            .saturating_sub(current);
        options.push((gain, 6, "handle_unchecked_exit", format!("{}:{}", exit.path, exit.line), format!(
            "Replace {} with an explicit error or absence path while preserving the caller's contract", exit.construct,
        )));
    }

    if let Some(item) = tip
        .unused_items
        .as_ref()
        .and_then(|items| items.first())
        .filter(|_| max_diff >= 8)
    {
        let gain = tip
            .projected_score(
                anchor,
                Projection {
                    fewer_unused: 1,
                    ..Projection::default()
                },
            )?
            .saturating_sub(current);
        options.push((gain, 8, "remove_unused_rust_item", format!("{}:{}", item.path, item.line), format!(
            "Rust reported {}. Confirm callers and other feature configurations before removing the item", item.message,
        )));
    }

    if let Some(flag) = tip
        .strict_flags
        .as_deref()
        .and_then(strictness::next_option)
        .filter(|_| max_diff >= 2)
    {
        let gain = tip
            .projected_score(
                anchor,
                Projection {
                    more_strict_flags: 1,
                    ..Projection::default()
                },
            )?
            .saturating_sub(current);
        options.push((
            gain,
            2,
            "tighten_typescript_option",
            format!("tsconfig.json:{flag}"),
            format!(
                "Enable {flag} in the root compilerOptions and preserve the existing project scope",
            ),
        ));
    }

    let (gain, _, kind, subject, detail) =
        options.into_iter().filter(|(gain, ..)| *gain > 0).max_by(
            |(gain, cost, kind, subject, _),
             (other_gain, other_cost, other_kind, other_subject, _)| {
                (u64::from(*gain) * u64::from(*other_cost))
                    .cmp(&(u64::from(*other_gain) * u64::from(*cost)))
                    .then_with(|| other_cost.cmp(cost))
                    .then_with(|| other_kind.cmp(kind))
                    .then_with(|| other_subject.cmp(subject))
            },
        )?;
    Some(Candidate {
        based_on: id,
        kind: kind.into(),
        subject,
        detail,
        max_diff_lines: max_diff,
        expected_gain_basis_points: Some(gain),
    })
}
