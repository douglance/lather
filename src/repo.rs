use crate::integrity::{RustGuard, policy_path, protected_rust, test_path};
use crate::model::{Error, Policy, Result, WithCode};
use crate::{process, strictness};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub struct Repo {
    pub id: String,
    pub worktree: String,
    pub root: PathBuf,
    pub profile: Profile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Rust,
    TypeScript,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    repos: Vec<Registration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    id: String,
    worktrees: Vec<Worktree>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Worktree {
    id: String,
    root: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    pub commit: String,
    pub dirty: bool,
}

pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl Repo {
    pub fn resolve(config: &Path, repo: &str, worktree: &str) -> Result<Self> {
        if !cfg!(target_os = "linux") {
            return Err(Error::new(
                "EXECUTION_ENVIRONMENT_DENIED",
                "Run repository checks on a Linux host",
            ));
        }
        if !valid_id(repo) || !valid_id(worktree) {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Invalid repository or worktree ID",
            ));
        }
        let registry = std::fs::read_to_string(config).code("REGISTRY_IO")?;
        let registry: Registry = toml::from_str(&registry).code("CONFIG_INVALID")?;
        if registry.schema_version != 1 {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Registry schema_version must be 1",
            ));
        }
        let registration = unique(
            registry.repos.into_iter().filter(|entry| entry.id == repo),
            "REPO_UNKNOWN",
        )?;
        let entry = unique(
            registration
                .worktrees
                .into_iter()
                .filter(|entry| entry.id == worktree),
            "WORKTREE_UNKNOWN",
        )?;
        if !entry.root.is_absolute() {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Registered worktree roots must be absolute",
            ));
        }
        let root = entry.root.canonicalize().code("WORKTREE_UNKNOWN")?;
        let rust = ["Cargo.toml", "Cargo.lock"]
            .iter()
            .all(|path| root.join(path).is_file());
        let typescript = ["package.json", "package-lock.json", "tsconfig.json"]
            .iter()
            .all(|path| root.join(path).is_file());
        let profile = match (rust, typescript) {
            (true, false) => Profile::Rust,
            (false, true) => Profile::TypeScript,
            (true, true) => {
                return Err(Error::new(
                    "UNSUPPORTED_PROJECT",
                    "Mixed Rust and TypeScript roots need separate worktree profiles",
                ));
            }
            (false, false) => {
                return Err(Error::new(
                    "UNSUPPORTED_PROJECT",
                    "The root needs Cargo.toml and Cargo.lock, or package.json, package-lock.json, and tsconfig.json",
                ));
            }
        };
        let result = Self {
            id: repo.into(),
            worktree: worktree.into(),
            root,
            profile,
        };
        if Path::new(result.git(&["rev-parse", "--show-toplevel"])?.trim()) != result.root {
            return Err(Error::new(
                "WORKTREE_UNKNOWN",
                "Registration must identify the Git worktree root",
            ));
        }
        Ok(result)
    }

    pub fn git(&self, args: &[&str]) -> Result<String> {
        process::run(&self.root, "git", args, Duration::from_secs(30))?.checked("git")
    }

    pub fn identity(&self) -> Result<Identity> {
        let commit = self.git(&["rev-parse", "--verify", "HEAD"])?.trim().into();
        let dirty = !self
            .git(&[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=none",
            ])?
            .is_empty();
        Ok(Identity { commit, dirty })
    }

    pub fn policy(&self) -> Result<Policy> {
        let policy: Policy = match std::fs::read_to_string(self.root.join("hygiene.toml")) {
            Ok(text) => toml::from_str(&text).code("CONFIG_INVALID")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Policy::default(),
            Err(error) => return Err(Error::new("CONFIG_INVALID", error.to_string())),
        };
        if policy.schema_version != 1 || policy.file_lines < 20 || policy.max_diff_lines == 0 {
            return Err(Error::new(
                "CONFIG_INVALID",
                "Use schema_version = 1, file_lines >= 20, and max_diff_lines > 0",
            ));
        }
        Ok(policy)
    }

    pub fn sources(&self) -> Result<impl Iterator<Item = Result<(String, String)>> + '_> {
        let list = match self.profile {
            Profile::Rust => self.git(&["ls-files", "-z", "--", "*.rs"]),
            Profile::TypeScript => self.git(&[
                "ls-files", "-z", "--", "*.ts", "*.tsx", "*.mts", "*.cts", "*.js", "*.jsx",
                "*.mjs", "*.cjs",
            ]),
        }?;
        let paths: Vec<_> = list
            .split_terminator('\0')
            .filter(|path| !test_path(path) && !policy_path(path) && !path.ends_with(".d.ts"))
            .map(String::from)
            .collect();
        Ok(paths.into_iter().filter_map(|path| {
            let source = self.root.join(&path);
            match source.symlink_metadata() {
                Ok(metadata) if metadata.is_file() => Some(
                    std::fs::read_to_string(source)
                        .code("WORKTREE_IO")
                        .map(|text| (path, text)),
                ),
                Ok(_) => None,
                Err(error) => Some(Err(Error::new("WORKTREE_IO", format!("{path}: {error}")))),
            }
        }))
    }

    pub fn node_tests(&self) -> Result<Vec<String>> {
        Ok(self
            .git(&["ls-files", "-z"])?
            .split_terminator('\0')
            .filter(|path| {
                let Some((base, extension)) = path.rsplit_once('.') else {
                    return false;
                };
                matches!(extension, "js" | "mjs" | "cjs" | "ts" | "mts" | "cts")
                    && (test_path(path) || base.ends_with(".test") || base.ends_with(".spec"))
            })
            .filter(|path| {
                let Some((name, _)) = path
                    .rsplit('/')
                    .next()
                    .and_then(|name| name.rsplit_once('.'))
                else {
                    return false;
                };
                name.ends_with(".test")
                    || name.ends_with(".spec")
                    || name.starts_with("test-")
                    || name.ends_with("-test")
                    || name == "test"
            })
            .map(|path| format!("./{path}"))
            .collect())
    }

    pub fn diff(&self, before: &str, after: &str) -> Result<Diff> {
        let base = self.git(&["merge-base", before, after])?;
        if base.trim() != before || before == after {
            return Err(Error::new(
                "STALE_CANDIDATE",
                "Candidate must be a new descendant of the comparison commit",
            ));
        }
        let changes = self.git(&[
            "diff",
            "--numstat",
            "--no-renames",
            "-z",
            before,
            after,
            "--",
        ])?;
        let mut diff = Diff::default();
        for change in changes.split_terminator('\0') {
            let mut fields = change.splitn(3, '\t');
            let (Some(add), Some(del), Some(path)) = (fields.next(), fields.next(), fields.next())
            else {
                return Err(Error::new("OUTPUT_UNPARSEABLE", "Could not parse Git diff"));
            };
            if add == "-" || del == "-" {
                diff.protected.insert(path.into());
                continue;
            }
            diff.lines += add.parse::<u64>().code("OUTPUT_UNPARSEABLE")?
                + del.parse::<u64>().code("OUTPUT_UNPARSEABLE")?;
            if self.profile == Profile::TypeScript && path == "tsconfig.json" {
                let old = self.git(&["show", &format!("{before}:{path}")])?;
                let new = self.git(&["show", &format!("{after}:{path}")])?;
                if !strictness::tightening(&old, &new) {
                    diff.protected.insert(path.into());
                }
            } else if policy_path(path) || (test_path(path) && self.has_path(before, path)?) {
                diff.protected.insert(path.into());
            } else if path.ends_with(".rs") && !test_path(path) {
                let before = self.rust_guard(before, path)?;
                let after = self.rust_guard(after, path)?;
                if after.weakens(&before) {
                    diff.protected.insert(path.into());
                }
            }
        }
        Ok(diff)
    }

    fn has_path(&self, commit: &str, path: &str) -> Result<bool> {
        self.git(&[
            "--literal-pathspecs",
            "ls-tree",
            "--name-only",
            commit,
            "--",
            path,
        ])
        .map(|result| !result.is_empty())
    }

    fn rust_guard(&self, commit: &str, path: &str) -> Result<RustGuard> {
        if self.has_path(commit, path)? {
            protected_rust(&self.git(&["show", &format!("{commit}:{path}")])?)
        } else {
            Ok(RustGuard::default())
        }
    }
}

fn unique<T>(mut entries: impl Iterator<Item = T>, code: &'static str) -> Result<T> {
    let entry = entries
        .next()
        .ok_or_else(|| Error::new(code, "No matching registration"))?;
    if entries.next().is_some() {
        return Err(Error::new("CONFIG_INVALID", "Duplicate registry ID"));
    }
    Ok(entry)
}

#[derive(Default)]
pub struct Diff {
    pub lines: u64,
    pub protected: BTreeSet<String>,
}
