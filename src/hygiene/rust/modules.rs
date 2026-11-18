mod cargo_graph;

use super::test_only;
use crate::hygiene::{Report, architecture, source::Facts};
use crate::model::Result;
use crate::repo::Repo;
use crate::rust_policy;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::visit::{self, Visit};

type Files<'a> = BTreeMap<Vec<String>, &'a str>;

pub fn measure(
    repo: &Repo,
    parsed: &BTreeMap<&str, syn::File>,
    facts: &mut BTreeMap<String, Facts>,
    enforce_cycles: bool,
    report: &mut Report,
) -> Result<()> {
    let metadata = rust_policy::metadata(repo)?;
    let crates = cargo_graph::crate_edges(repo, &metadata, facts);
    report
        .scope
        .insert("rust-graph".into(), "workspace-dependencies-v1".into());
    let mut module_edges = BTreeMap::<String, Facts>::new();
    for package in metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
    {
        for target in &package.targets {
            let Some(root) = relative(repo, &target.src_path) else {
                continue;
            };
            if !parsed.contains_key(root) {
                continue;
            }
            let mut files = BTreeMap::new();
            let mut references = Vec::new();
            collect_file(
                parsed,
                root,
                Path::new(root).parent().unwrap_or(Path::new("")),
                &[],
                &mut files,
                &mut references,
                &mut BTreeSet::new(),
            );
            for (path, module, names, line) in references {
                if let Some(target) =
                    module_target(&files, &module, &names).filter(|target| *target != path)
                {
                    module_edges
                        .entry(path.into())
                        .or_default()
                        .edges
                        .entry(target.into())
                        .or_insert(line);
                }
            }
        }
    }
    let cycles = architecture::cycles(&module_edges);
    if enforce_cycles {
        for part in &cycles {
            report.record(
                "S9",
                part[0],
                1,
                format!("Rust module cycle among {}", part.join(", ")),
                1,
            );
        }
    }
    report
        .inventory
        .insert("S9.rust_modules".into(), serde_json::json!(cycles));
    for (path, module) in module_edges {
        if let Some(file) = facts.get_mut(&path) {
            file.module_edges.extend(module.edges);
        }
    }
    report
        .inventory
        .insert("S8.workspace_packages".into(), serde_json::json!(crates));
    Ok(())
}

fn relative<'a>(repo: &Repo, path: &'a Path) -> Option<&'a str> {
    path.strip_prefix(&repo.root).ok()?.to_str()
}

type Reference<'a> = (&'a str, Vec<String>, Vec<String>, usize);

fn collect_file<'a>(
    parsed: &BTreeMap<&'a str, syn::File>,
    path: &'a str,
    directory: &Path,
    module: &[String],
    files: &mut Files<'a>,
    references: &mut Vec<Reference<'a>>,
    stack: &mut BTreeSet<&'a str>,
) {
    if let Some(ast) = parsed.get(path)
        && stack.insert(path)
    {
        let mut context = Module {
            parsed,
            path,
            files,
            references,
            stack,
        };
        context.collect(&ast.items, directory, module);
        context.stack.remove(path);
    }
}

struct Module<'a, 'b> {
    parsed: &'b BTreeMap<&'a str, syn::File>,
    path: &'a str,
    files: &'b mut Files<'a>,
    references: &'b mut Vec<Reference<'a>>,
    stack: &'b mut BTreeSet<&'a str>,
}

impl Module<'_, '_> {
    fn collect(&mut self, items: &[syn::Item], directory: &Path, module: &[String]) {
        self.files.insert(module.to_vec(), self.path);
        for item in items {
            if let syn::Item::Mod(child) = item {
                if test_only(&child.attrs) {
                    continue;
                }
                let name = child.ident.to_string();
                let child_module = [module, std::slice::from_ref(&name)].concat();
                let child_folder = directory.join(&name);
                if let Some((_, items)) = &child.content {
                    self.collect(items, &child_folder, &child_module);
                } else {
                    let explicit = child.attrs.iter().find_map(module_path);
                    let candidates = explicit.map_or_else(
                        || {
                            vec![
                                directory.join(format!("{name}.rs")),
                                child_folder.join("mod.rs"),
                            ]
                        },
                        |file| vec![directory.join(file)],
                    );
                    if let Some((&file, _)) = candidates
                        .iter()
                        .filter_map(|path| path.to_str())
                        .find_map(|path| self.parsed.get_key_value(path))
                    {
                        collect_file(
                            self.parsed,
                            file,
                            &child_folder,
                            &child_module,
                            self.files,
                            self.references,
                            self.stack,
                        );
                    }
                }
            } else {
                let mut paths = Paths {
                    path: self.path,
                    module,
                    references: self.references,
                };
                paths.visit_item(item);
            }
        }
    }
}

fn module_path(attr: &syn::Attribute) -> Option<String> {
    let meta = attr.meta.require_name_value().ok()?;
    if !meta.path.is_ident("path") {
        return None;
    }
    let syn::Expr::Lit(value) = &meta.value else {
        return None;
    };
    let syn::Lit::Str(path) = &value.lit else {
        return None;
    };
    Some(path.value())
}

struct Paths<'a, 'b> {
    path: &'a str,
    module: &'b [String],
    references: &'b mut Vec<Reference<'a>>,
}

impl Paths<'_, '_> {
    fn add(&mut self, names: Vec<String>, line: usize) {
        if names.len() > 1 {
            self.references
                .push((self.path, self.module.to_vec(), names, line));
        }
    }
}

impl<'ast> Visit<'ast> for Paths<'_, '_> {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(first) = path.segments.first() {
            self.add(
                path.segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
                first.ident.span().start().line,
            );
        }
        visit::visit_path(self, path);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        fn expand(
            tree: &syn::UseTree,
            names: &mut Vec<String>,
            out: &mut Paths<'_, '_>,
            line: usize,
        ) {
            match tree {
                syn::UseTree::Path(path) => {
                    names.push(path.ident.to_string());
                    expand(&path.tree, names, out, line);
                    names.pop();
                }
                syn::UseTree::Group(group) => {
                    for child in &group.items {
                        expand(child, names, out, line);
                    }
                }
                syn::UseTree::Name(name) => {
                    let mut branch = names.clone();
                    branch.push(name.ident.to_string());
                    out.add(branch, line);
                }
                syn::UseTree::Rename(rename) => {
                    let mut branch = names.clone();
                    branch.push(rename.ident.to_string());
                    out.add(branch, line);
                }
                syn::UseTree::Glob(_) => {
                    let mut branch = names.clone();
                    branch.push("*".into());
                    out.add(branch, line);
                }
            }
        }
        expand(
            &item.tree,
            &mut Vec::new(),
            self,
            item.use_token.span.start().line,
        );
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !test_only(&item.attrs) {
            visit::visit_item_fn(self, item);
        }
    }
}

fn module_target<'a>(files: &Files<'a>, module: &[String], names: &[String]) -> Option<&'a str> {
    let mut prefix = module.to_vec();
    let mut rest = names;
    match rest.first()?.as_str() {
        "crate" => {
            prefix.clear();
            rest = &rest[1..];
        }
        "self" => rest = &rest[1..],
        "super" => {
            while rest.first().is_some_and(|part| part == "super") {
                prefix.pop()?;
                rest = &rest[1..];
            }
        }
        _ => {}
    }
    let before = prefix.len();
    prefix.extend_from_slice(rest);
    while prefix.len() > before {
        if let Some(path) = files.get(&prefix) {
            return Some(path);
        }
        prefix.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_crate_and_parent_paths_to_deepest_known_module() {
        let files = BTreeMap::from([
            (vec!["a".into()], "a"),
            (vec!["a".into(), "child".into()], "a/child"),
            (vec!["b".into()], "b"),
        ]);
        let module = vec!["a".into(), "child".into()];
        let target = |parts: &[&str]| {
            module_target(
                &files,
                &module,
                &parts.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(target(&["crate", "b", "func"]), Some("b"));
        assert_eq!(target(&["super", "child", "func"]), Some("a/child"));
        assert_eq!(target(&["std", "io", "read"]), None);
    }
}
