use super::{Boundary, Report, source::Facts};
use std::collections::{BTreeMap, BTreeSet};

pub fn measure(facts: &BTreeMap<String, Facts>, rules: &[Boundary], report: &mut Report) {
    let mut graph = BTreeMap::new();
    for (path, file) in facts {
        let edges: BTreeSet<_> = file.edges.keys().map(String::as_str).collect();
        for (target, line) in file.edges.iter().chain(&file.module_edges) {
            if rules
                .iter()
                .any(|rule| prefix(path, &rule.from) && prefix(target, &rule.forbid))
            {
                report.record(
                    "S8",
                    path,
                    *line,
                    format!("Forbidden dependency on {target}"),
                    1,
                );
            }
        }
        graph.insert(path.as_str(), edges);
    }
    for part in components(&graph) {
        let path = part[0];
        report.record(
            "S9",
            path,
            1,
            format!("Dependency cycle among {}", part.join(", ")),
            1,
        );
    }
}

pub(super) fn cycles(graph: &BTreeMap<String, Facts>) -> Vec<Vec<&str>> {
    let graph = graph
        .iter()
        .map(|(path, file)| {
            (
                path.as_str(),
                file.edges.keys().map(String::as_str).collect(),
            )
        })
        .collect();
    components(&graph)
}

fn prefix(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn components<'a>(graph: &BTreeMap<&'a str, BTreeSet<&'a str>>) -> Vec<Vec<&'a str>> {
    let mut forward = Vec::new();
    let mut visited = BTreeSet::new();
    let mut reverse: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (&path, edges) in graph {
        for &edge in edges {
            if graph.contains_key(edge) {
                reverse.entry(edge).or_default().insert(path);
            }
        }
    }
    for &path in graph.keys() {
        let mut stack = vec![(path, false)];
        while let Some((next, finished)) = stack.pop() {
            if finished {
                forward.push(next);
            } else if visited.insert(next) {
                stack.push((next, true));
                if let Some(edges) = graph.get(next) {
                    stack.extend(
                        edges
                            .iter()
                            .filter(|&&edge| graph.contains_key(edge))
                            .map(|&edge| (edge, false)),
                    );
                }
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut cycles = Vec::new();
    for path in forward.into_iter().rev() {
        let mut members = Vec::new();
        let mut stack = vec![path];
        while let Some(next) = stack.pop() {
            if visited.insert(next) {
                members.push(next);
                if let Some(edges) = reverse.get(next) {
                    stack.extend(edges);
                }
            }
        }
        if members.len() > 1 {
            members.sort_unstable();
            cycles.push(members);
        }
    }
    cycles.sort_unstable();
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_disjoint_cycles_and_path_segment_boundaries() {
        let graph = BTreeMap::from([
            ("a", BTreeSet::from(["b"])),
            ("b", BTreeSet::from(["a", "x"])),
            ("x", BTreeSet::from(["y"])),
            ("y", BTreeSet::from(["x", "z"])),
            ("z", BTreeSet::from(["z"])),
        ]);
        assert_eq!(components(&graph), vec![vec!["a", "b"], vec!["x", "y"]]);
        assert!(prefix("src/ui/widget", "src/ui/"));
        assert!(!prefix("src/ui2/widget", "src/ui/"));
    }
}
