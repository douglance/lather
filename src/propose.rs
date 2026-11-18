use crate::model::{Candidate, Projection, Snapshot};
use crate::strictness;

type Ranked = (u32, u32, &'static str, String, String);

pub fn highest_gain(id: i64, tip: &Snapshot, anchor: &Snapshot) -> Option<Candidate> {
    let current = tip.score(anchor)?;
    let max_diff = tip.policy.max_diff_lines;
    let mut options = Vec::new();

    size_and_rust_debt(tip, anchor, current, max_diff, &mut options)?;

    compiler_options(tip, anchor, current, max_diff, &mut options)?;

    if let Some(report) = &tip.hygiene {
        for (id, _, observation) in report.scored(&tip.policy.hygiene) {
            let Some(site) = observation.findings.first() else {
                continue;
            };
            let (kind, detail, cost) = match id.as_str() {
                "S2.rust" => (
                    "tighten_rust_lint",
                    "Tighten this crate-root lint with a deny or forbid attribute",
                    2,
                ),
                "S3" => (
                    "remove_type_escape",
                    "Replace the escape hatch with explicit checked handling",
                    8,
                ),
                "S5" => (
                    "remove_lint_suppression",
                    "Fix the underlying lint before removing the suppression",
                    8,
                ),
                "S10" => (
                    "use_design_system",
                    "Use the configured design-system alternative",
                    8,
                ),
                "S11" => (
                    "consolidate_duplicate",
                    "Share one implementation while preserving both callers",
                    16,
                ),
                "S12.external"
                    if !site.path.ends_with("Cargo.toml")
                        && !site.path.ends_with("package.json") =>
                {
                    (
                        "remove_unused_code",
                        "Confirm callers and entry points before removing unreachable code or exports",
                        8,
                    )
                }
                "S13" | "S13.external" => (
                    "resolve_maintenance_marker",
                    "Resolve the underlying issue and remove the obsolete marker",
                    6,
                ),
                "S19" => (
                    "reduce_oversized_function",
                    "Extract one cohesive operation while preserving behavior",
                    24,
                ),
                "S24" | "S24.external" => (
                    "handle_error_explicitly",
                    "Replace the placeholder or handle failure explicitly",
                    8,
                ),
                "S25" | "S25.external" => (
                    "fix_accessibility",
                    "Resolve the reported accessibility violation using the intended semantics",
                    8,
                ),
                _ => continue,
            };
            if cost > max_diff {
                continue;
            }
            let reduction = if id == "S19" {
                8.min(observation.debt)
            } else {
                1
            };
            let gain = tip
                .projected_score(
                    anchor,
                    Projection {
                        hygiene: Some((id.clone(), reduction)),
                        ..Projection::default()
                    },
                )?
                .saturating_sub(current);
            options.push((
                gain,
                cost,
                kind,
                format!("{}:{}", site.path, site.line),
                format!("{detail}: {}", site.detail),
            ));
        }
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

fn size_and_rust_debt(
    tip: &Snapshot,
    anchor: &Snapshot,
    current: u32,
    max_diff: u32,
    options: &mut Vec<Ranked>,
) -> Option<()> {
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

    Some(())
}

fn compiler_options(
    tip: &Snapshot,
    anchor: &Snapshot,
    current: u32,
    max_diff: u32,
    options: &mut Vec<Ranked>,
) -> Option<()> {
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

    if let Some(path) = tip
        .rust_unsafe_policy
        .as_ref()
        .and_then(|roots| {
            roots
                .iter()
                .find_map(|(path, enforced)| (!enforced).then_some(path))
        })
        .filter(|_| max_diff >= 2)
    {
        let gain = tip
            .projected_score(
                anchor,
                Projection {
                    fewer_unprotected_rust_roots: 1,
                    ..Projection::default()
                },
            )?
            .saturating_sub(current);
        options.push((
            gain,
            2,
            "forbid_unsafe_code",
            path.clone(),
            "Review unsafe usage and add #![forbid(unsafe_code)] to the crate root".into(),
        ));
    }

    Some(())
}
