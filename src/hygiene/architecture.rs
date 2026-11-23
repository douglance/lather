use super::{Boundary, Report, source::Facts};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn measure(facts: &BTreeMap<String, Facts>, rules: &[Boundary], report: &mut Report) {
    for (path, file) in facts {
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
    }
    let cycles = cycles(facts);
    for cycle in &cycles {
        cycle.record(report, "Dependency cycle");
    }
    if !cycles.is_empty() {
        report
            .inventory
            .insert("S9.cycle_edges".into(), serde_json::json!(cycles));
    }
}

#[derive(Serialize)]
pub(super) struct Cycle<'a> {
    pub members: Vec<&'a str>,
    edges: Vec<Edge<'a>>,
}

#[derive(Serialize)]
struct Edge<'a> {
    from: &'a str,
    line: usize,
    to: &'a str,
}

impl Cycle<'_> {
    pub fn record(&self, report: &mut Report, label: &str) {
        let first = &self.edges[0];
        let path = self
            .edges
            .iter()
            .map(|edge| format!("{}:{}", edge.from, edge.line))
            .chain(std::iter::once(first.from.to_owned()))
            .collect::<Vec<_>>()
            .join(" -> ");
        report.record("S9", first.from, first.line, format!("{label}: {path}"), 1);
    }
}

pub(super) fn cycles(facts: &BTreeMap<String, Facts>) -> Vec<Cycle<'_>> {
    let graph = facts
        .iter()
        .map(|(path, file)| {
            (
                path.as_str(),
                file.edges.keys().map(String::as_str).collect(),
            )
        })
        .collect();
    components(&graph)
        .into_iter()
        .map(|members| {
            let edges = cycle_path(&graph, &members)
                .windows(2)
                .map(|pair| Edge {
                    from: pair[0],
                    line: facts[pair[0]].edges[pair[1]],
                    to: pair[1],
                })
                .collect();
            Cycle { members, edges }
        })
        .collect()
}

// Breadth-first traversal gives the shortest loop through the first member.
fn cycle_path<'a>(
    graph: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    members: &[&'a str],
) -> Vec<&'a str> {
    let start = members[0];
    let mut queue = VecDeque::from([start]);
    let mut parents = BTreeMap::from([(start, start)]);
    while let Some(from) = queue.pop_front() {
        for &to in &graph[from] {
            if to == start && from != start {
                let mut path = vec![start, from];
                let mut current = from;
                while current != start {
                    current = parents[current];
                    path.push(current);
                }
                path.reverse();
                return path;
            }
            if members.binary_search(&to).is_ok()
                && let std::collections::btree_map::Entry::Vacant(parent) = parents.entry(to)
            {
                parent.insert(from);
                queue.push_back(to);
            }
        }
    }
    unreachable!("a multi-node strongly connected component contains a loop through every member")
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
    fn reports_a_real_cycle_with_reference_lines_and_one_gate_violation() {
        let file = |edges: &[(&str, usize)]| Facts {
            edges: edges.iter().map(|&(to, line)| (to.into(), line)).collect(),
            ..Facts::default()
        };
        let facts = BTreeMap::from([
            (
                "a.ts".into(),
                file(&[("a.ts", 2), ("c.ts", 5), ("leaf.ts", 6)]),
            ),
            ("b.ts".into(), file(&[("c.ts", 9)])),
            ("c.ts".into(), file(&[("a.ts", 11), ("b.ts", 12)])),
            ("leaf.ts".into(), file(&[])),
        ]);
        let mut report = Report::default();
        measure(&facts, &[], &mut report);

        assert_eq!(report.signals["S9"].debt, 1);
        let finding = &report.signals["S9"].findings[0];
        assert_eq!(finding.path, "a.ts");
        assert_eq!(finding.line, 5);
        assert_eq!(
            finding.detail,
            "Dependency cycle: a.ts:5 -> c.ts:11 -> a.ts"
        );
        assert_eq!(
            report.inventory["S9.cycle_edges"],
            serde_json::json!([{
                "members": ["a.ts", "b.ts", "c.ts"],
                "edges": [
                    {"from": "a.ts", "line": 5, "to": "c.ts"},
                    {"from": "c.ts", "line": 11, "to": "a.ts"}
                ]
            }])
        );
    }

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
