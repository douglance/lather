use crate::model::{Error, Result, WithCode};
use crate::process;
use crate::repo::{Profile, Repo};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    workspace_members: BTreeSet<String>,
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    manifest_path: PathBuf,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
    src_path: PathBuf,
}

pub fn observe(repo: &Repo, issues: &mut Vec<String>) -> Option<BTreeMap<String, bool>> {
    if repo.profile != Profile::Rust {
        return None;
    }
    match workspace_policy(repo) {
        Ok(roots) => Some(roots),
        Err(error) => {
            issues.push(format!("Rust unsafe policy: {error}"));
            None
        }
    }
}

fn workspace_policy(repo: &Repo) -> Result<BTreeMap<String, bool>> {
    let metadata = process::run(
        &repo.root,
        "cargo",
        &[
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
            "--locked",
        ],
        Duration::from_secs(30),
    )?
    .checked("cargo metadata")?;
    let metadata: Metadata = serde_json::from_str(&metadata).code("RUST_POLICY_METADATA")?;
    if metadata.workspace_root != repo.root {
        return Err(Error::new(
            "RUST_POLICY_SCOPE",
            "Cargo workspace root differs from the registered worktree",
        ));
    }
    let tracked = repo.git(&["ls-files", "-z", "--cached"])?;
    let tracked = tracked.split_terminator('\0').collect();
    let root_manifest = manifest(repo, &tracked, &repo.root.join("Cargo.toml"))?;
    let workspace_lints = root_manifest
        .get("workspace")
        .and_then(|value| value.get("lints"));
    let mut roots: BTreeMap<String, bool> = BTreeMap::new();
    for package in metadata.packages {
        if !metadata.workspace_members.contains(&package.id) {
            continue;
        }
        let targets = package.targets.iter().filter(|target| {
            target.kind.iter().any(|kind| {
                matches!(
                    kind.as_str(),
                    "bin" | "lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro"
                )
            })
        });
        let manifest = manifest(repo, &tracked, &package.manifest_path)?;
        let lints = manifest.get("lints");
        let manifest_forbids = if lints
            .and_then(|value| value.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            forbids_in_manifest(workspace_lints)
        } else {
            forbids_in_manifest(lints)
        };
        for target in targets {
            let path = tracked_path(repo, &tracked, &target.src_path)?;
            let forbids = manifest_forbids || forbids_in_source(&target.src_path)?;
            roots
                .entry(path.into())
                .and_modify(|all| *all &= forbids)
                .or_insert(forbids);
        }
    }
    Ok(roots)
}

fn tracked_path<'a>(repo: &Repo, tracked: &BTreeSet<&str>, path: &'a Path) -> Result<&'a str> {
    let relative = path.strip_prefix(&repo.root).ok().and_then(Path::to_str);
    if let Some(relative) = relative
        && tracked.contains(relative)
        && path.canonicalize().is_ok_and(|resolved| resolved == path)
        && path.is_file()
    {
        return Ok(relative);
    }
    Err(Error::new(
        "RUST_POLICY_SCOPE",
        format!(
            "{} is not a tracked regular file in the worktree",
            path.display()
        ),
    ))
}

fn manifest(repo: &Repo, tracked: &BTreeSet<&str>, path: &Path) -> Result<toml::Value> {
    tracked_path(repo, tracked, path)?;
    let text = std::fs::read_to_string(path).code("RUST_POLICY_IO")?;
    toml::from_str(&text).code("RUST_POLICY_MANIFEST")
}

fn forbids_in_manifest(lints: Option<&toml::Value>) -> bool {
    let Some(rule) = lints
        .and_then(|value| value.get("rust"))
        .and_then(|rust| rust.get("unsafe_code"))
    else {
        return false;
    };
    rule.as_str() == Some("forbid")
        || rule.get("level").and_then(toml::Value::as_str) == Some("forbid")
}

fn forbids_in_source(path: &Path) -> Result<bool> {
    let source = std::fs::read_to_string(path).code("RUST_POLICY_IO")?;
    let root = syn::parse_file(&source).map_err(|error| {
        Error::new("RUST_POLICY_SOURCE", format!("{}: {error}", path.display()))
    })?;
    Ok(forbids_at_crate_root(&root))
}

fn forbids_at_crate_root(root: &syn::File) -> bool {
    use syn::{Meta, Token, punctuated::Punctuated};
    root.attrs.iter().any(|attr| {
        matches!(attr.style, syn::AttrStyle::Inner(_))
            && attr.path().is_ident("forbid")
            && attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|args| {
                    args.iter()
                        .any(|arg| matches!(arg, Meta::Path(path) if path.is_ident("unsafe_code")))
                })
    })
}

#[cfg(test)]
#[path = "tests/rust_policy.rs"]
mod tests;
