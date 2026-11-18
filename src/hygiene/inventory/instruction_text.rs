use std::collections::BTreeSet;
use std::path::{Component, PathBuf};

pub(super) fn instruction_facts(body: &str, tracked: &BTreeSet<&str>) -> serde_json::Value {
    let points = [
        "@AGENTS.md",
        "./AGENTS.md",
        "[AGENTS.md](AGENTS.md)",
        "[AGENTS.md](./AGENTS.md)",
    ];
    let links: BTreeSet<_> = body
        .split("](")
        .skip(1)
        .filter_map(|chunk| {
            chunk
                .split_once(')')
                .map(|(link, _)| link.trim_matches(['<', '>']))
        })
        .filter(|link| !link.is_empty() && !link.starts_with(['#', '/']) && !link.contains(':'))
        .filter(|link| {
            let path = PathBuf::from(link.split('#').next().unwrap_or_default());
            let canonical: PathBuf = path
                .components()
                .filter(|part| *part != Component::CurDir)
                .collect();
            !canonical.to_str().is_some_and(|path| {
                tracked.contains(path)
                    || tracked.iter().any(|name| {
                        name.strip_prefix(path)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                    })
            })
        })
        .collect();
    let lines = body.lines().count();
    let commands = [
        "cargo build",
        "cargo test",
        "cargo check",
        "npm run build",
        "npm test",
        "npm run lint",
        "just verify",
        "make check",
    ];
    serde_json::json!({
        "lines": lines,
        "over_budget": lines.saturating_sub(200),
        "points_to_agents": points.iter().any(|candidate| body.trim() == *candidate),
        "unresolved_local_links": links,
        "recognized_commands": commands.iter().filter(|command| body.contains(**command)).collect::<Vec<_>>(),
    })
}

pub(super) fn prose_rules(text: &str) -> usize {
    let mut fenced = false;
    text.lines()
        .filter(|line| {
            let line = line.trim();
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
            }
            if fenced || !(line.starts_with("- ") || line.starts_with("* ")) {
                return false;
            }
            line.split(|c: char| !c.is_alphanumeric() && c != '\'')
                .any(|word| {
                    ["always", "never", "don't", "must"]
                        .iter()
                        .any(|keyword| word.eq_ignore_ascii_case(keyword))
                })
        })
        .count()
}
