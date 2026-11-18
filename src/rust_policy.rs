use crate::hygiene::Report;
use crate::model::{Error, Result, WithCode};
use crate::process;
use crate::repo::{Profile, Repo};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
pub(crate) struct Metadata {
    pub workspace_root: PathBuf,
    pub workspace_members: BTreeSet<String>,
    pub packages: Vec<Package>,
}

#[derive(Deserialize)]
pub(crate) struct Package {
    pub id: String,
    pub manifest_path: PathBuf,
    pub targets: Vec<Target>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
pub(crate) struct Dependency {
    pub name: String,
    pub kind: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Deserialize)]
pub(crate) struct Target {
    pub kind: Vec<String>,
    pub src_path: PathBuf,
}

const STRICT_LINTS: &[&str] = &["unused_must_use", "unreachable_code"];
type Policies = BTreeMap<String, BTreeMap<&'static str, bool>>;

pub fn observe(
    repo: &Repo,
    report: &mut Report,
    issues: &mut Vec<String>,
) -> Option<BTreeMap<String, bool>> {
    if repo.profile != Profile::Rust {
        return None;
    }
    report.signal("S2.rust");
    match workspace_policy(repo) {
        Ok(roots) => {
            let mut unsafe_policy = BTreeMap::new();
            for (path, rules) in roots {
                unsafe_policy.insert(path.clone(), rules["unsafe_code"]);
                for &rule in STRICT_LINTS {
                    if rules[rule] {
                        report.protections.insert(format!("{path}:{rule}"));
                    } else {
                        report.record("S2.rust", &path, 1, rule, 1);
                    }
                }
            }
            Some(unsafe_policy)
        }
        Err(error) => {
            issues.push(format!("Rust unsafe policy: {error}"));
            None
        }
    }
}

pub(crate) fn metadata(repo: &Repo) -> Result<Metadata> {
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
    Ok(metadata)
}

fn workspace_policy(repo: &Repo) -> Result<Policies> {
    let metadata = metadata(repo)?;
    let tracked = repo.git(&["ls-files", "-z", "--cached"])?;
    let tracked = tracked.split_terminator('\0').collect();
    let root_manifest = manifest(repo, &tracked, &repo.root.join("Cargo.toml"))?;
    let workspace_lints = root_manifest
        .get("workspace")
        .and_then(|value| value.get("lints"));
    let mut roots: Policies = BTreeMap::new();
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
        let lints = if lints
            .and_then(|value| value.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true)
        {
            workspace_lints
        } else {
            lints
        };
        for target in targets {
            let path = tracked_path(repo, &tracked, &target.src_path)?;
            let root = parse_source(&target.src_path)?;
            let policies = roots.entry(path.into()).or_default();
            let forbids = forbids_in_manifest(lints) || forbids_at_crate_root(&root);
            policies
                .entry("unsafe_code")
                .and_modify(|all| *all &= forbids)
                .or_insert(forbids);
            for &rule in STRICT_LINTS {
                let enabled = strict_at_root(&root, lint_level(lints, rule), rule);
                policies
                    .entry(rule)
                    .and_modify(|all| *all &= enabled)
                    .or_insert(enabled);
            }
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
    lint_level(lints, "unsafe_code") == Some("forbid")
}

fn lint_level<'a>(lints: Option<&'a toml::Value>, lint: &str) -> Option<&'a str> {
    let rule = lints?.get("rust")?.get(lint)?;
    rule.as_str().or_else(|| rule.get("level")?.as_str())
}

fn parse_source(path: &Path) -> Result<syn::File> {
    let source = std::fs::read_to_string(path).code("RUST_POLICY_IO")?;
    syn::parse_file(&source)
        .map_err(|error| Error::new("RUST_POLICY_SOURCE", format!("{}: {error}", path.display())))
}

fn forbids_at_crate_root(root: &syn::File) -> bool {
    levels(root, "unsafe_code").any(|level| level == "forbid")
}

fn strict_at_root(root: &syn::File, manifest: Option<&str>, lint: &str) -> bool {
    let mut effective = manifest.unwrap_or("warn").to_owned();
    for level in levels(root, lint) {
        if effective != "forbid" {
            effective = level;
        }
    }
    matches!(effective.as_str(), "deny" | "forbid")
}

fn levels<'a>(root: &'a syn::File, lint: &'a str) -> impl Iterator<Item = String> + 'a {
    use syn::{Meta, Token, punctuated::Punctuated};
    root.attrs.iter().filter_map(move |attr| {
        let level = attr.path().get_ident()?.to_string();
        if !matches!(attr.style, syn::AttrStyle::Inner(_))
            || !matches!(
                level.as_str(),
                "allow" | "expect" | "warn" | "deny" | "forbid"
            )
        {
            return None;
        }
        let args = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok()?;
        args.iter()
            .any(|arg| matches!(arg, Meta::Path(path) if path.is_ident(lint)))
            .then_some(level)
    })
}

#[cfg(test)]
#[path = "tests/rust_policy.rs"]
mod tests;
