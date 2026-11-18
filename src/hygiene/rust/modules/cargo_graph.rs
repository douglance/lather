use super::relative;
use crate::hygiene::source::Facts;
use crate::repo::Repo;
use crate::rust_policy::{Metadata, Package};
use std::collections::BTreeMap;

fn source_root<'a>(repo: &Repo, package: &'a Package) -> Option<&'a str> {
    let target = package
        .targets
        .iter()
        .find(|target| {
            target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                )
            })
        })
        .or_else(|| {
            package
                .targets
                .iter()
                .find(|target| target.kind.iter().any(|kind| kind == "bin"))
        })?;
    relative(repo, &target.src_path)
}

pub(super) fn crate_edges<'a>(
    repo: &Repo,
    metadata: &'a Metadata,
    facts: &mut BTreeMap<String, Facts>,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let packages: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
        .filter_map(|package| Some((package.manifest_path.parent()?, source_root(repo, package)?)))
        .collect();
    let mut graph = BTreeMap::new();
    for package in &metadata.packages {
        let Some(source) = source_root(repo, package) else {
            continue;
        };
        if !metadata.workspace_members.contains(&package.id) {
            continue;
        }
        let mut dependencies: Vec<_> = package
            .dependencies
            .iter()
            .filter(|dependency| dependency.kind.as_deref() != Some("dev"))
            .filter_map(|dependency| packages.get(dependency.path.as_deref()?).copied())
            .filter(|&target| target != source)
            .collect();
        dependencies.sort_unstable();
        dependencies.dedup();
        if let Some(file) = facts.get_mut(source) {
            file.edges
                .extend(dependencies.iter().map(|&target| (target.into(), 1)));
            file.module_edges.extend(
                package
                    .dependencies
                    .iter()
                    .map(|dependency| (format!("package:{}", dependency.name), 1)),
            );
        }
        graph.insert(source, dependencies);
    }
    graph
}
