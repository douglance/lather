use crate::model::{Error, Result, WithCode};
use crate::store::DATABASE_FILES;
use std::path::PathBuf;

const REGISTRY_FILES: &[&str] = &["repositories.toml", "washlist.toml", "repos.toml"];

pub(super) fn resolve() -> Result<(PathBuf, PathBuf)> {
    let state = match path_override(&["LATHER_HOME", "HYGIENE_CLIMB_HOME"]) {
        Some(path) => path,
        None => data_directory("XDG_STATE_HOME", ".local/state", DATABASE_FILES)?,
    };
    let registry = match path_override(&[
        "LATHER_REPOS",
        "LATHER_WASHLIST",
        "LATHER_REGISTRY",
        "HYGIENE_CLIMB_REGISTRY",
    ]) {
        Some(path) => path,
        None => {
            let directory = data_directory("XDG_CONFIG_HOME", ".config", REGISTRY_FILES)?;
            let mut registry = directory.join(REGISTRY_FILES[0]);
            for filename in REGISTRY_FILES {
                let candidate = directory.join(filename);
                if candidate.try_exists().code("PATH_IO")? {
                    registry = candidate;
                    break;
                }
            }
            registry
        }
    };
    Ok((state, registry))
}

fn path_override(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(std::env::var_os).map(PathBuf::from)
}

fn data_directory(xdg_key: &str, fallback: &str, filenames: &[&str]) -> Result<PathBuf> {
    let parent = match std::env::var_os(xdg_key) {
        Some(parent) => PathBuf::from(parent),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(fallback))
            .ok_or_else(|| {
                Error::new(
                    "CONFIG_INVALID",
                    "HOME is required when XDG paths are unset",
                )
            })?,
    };
    let lather = parent.join("lather");
    let legacy = parent.join("hygiene-climb");
    for filename in filenames {
        if lather.join(filename).try_exists().code("PATH_IO")? {
            return Ok(lather);
        }
    }
    for filename in filenames {
        if legacy.join(filename).try_exists().code("PATH_IO")? {
            return Ok(legacy);
        }
    }
    Ok(lather)
}
