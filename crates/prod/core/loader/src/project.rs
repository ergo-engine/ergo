//! project
//!
//! Purpose:
//! - Discover a filesystem-backed Ergo project root and parse `ergo.toml` into loader-owned project/profile data.
//! - Resolve authored project profiles into path-backed run inputs consumed by higher prod layers.
//!
//! Owns:
//! - Upward filesystem discovery for `ergo.toml`.
//! - TOML decode of the path-backed project manifest and profile schema.
//! - Resolution of authored relative paths and ingress config into `ResolvedProject*` values.
//!
//! Does not own:
//! - In-memory project/profile products owned by SDK-side configuration.
//! - Graph decode, cluster discovery, semantic validation, or runtime execution policy.
//! - Host interpretation of adapter/egress/capture semantics beyond carrying resolved paths forward.
//!
//! Connects to:
//! - `lib.rs` for the public loader re-export surface.
//! - `sdk-rust` for filesystem project loading and profile-plan preparation.
//! - Loader docs and `ergo.toml` authoring as the path-backed project contract.
//!
//! Safety notes:
//! - `ProjectManifest` and `ProjectProfile` are unresolved authored config; `ResolvedProject*` joins authored strings against the project root, but does not reject absolute paths (callers control their own `ergo.toml`).
//! - Profile resolution enforces exactly one ingress source so callers do not inherit ambiguous fixture/process behavior.
//! - Loader project resolution stays path-backed and transport/config only; it does not become semantic authority for graph validity or run policy.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    ProjectRootNotFound { start: PathBuf },
    ProjectRead { path: PathBuf, detail: String },
    ProjectParse { path: PathBuf, detail: String },
    ProfileNotFound { name: String },
    ProfileInvalid { name: String, detail: String },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectRootNotFound { start } => write!(
                f,
                "could not discover project root from '{}' (no ergo.toml found)",
                start.display()
            ),
            Self::ProjectRead { path, detail } => {
                write!(f, "failed to read '{}': {detail}", path.display())
            }
            Self::ProjectParse { path, detail } => {
                write!(f, "failed to parse '{}': {detail}", path.display())
            }
            Self::ProfileNotFound { name } => {
                write!(f, "project profile '{name}' does not exist")
            }
            Self::ProfileInvalid { name, detail } => {
                write!(f, "project profile '{name}' is invalid: {detail}")
            }
        }
    }
}

impl std::error::Error for ProjectError {}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    pub name: String,
    pub version: String,
    pub profiles: BTreeMap<String, ProjectProfile>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ProjectProfile {
    pub graph: String,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub fixture: Option<String>,
    #[serde(default)]
    pub ingress: Option<ProjectIngress>,
    #[serde(default)]
    pub egress: Option<String>,
    #[serde(default)]
    pub capture_output: Option<String>,
    #[serde(default)]
    pub pretty_capture: Option<bool>,
    #[serde(default, with = "duration_option_serde")]
    pub max_duration: Option<Duration>,
    #[serde(default)]
    pub max_events: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectIngress {
    Process { command: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProjectIngress {
    Fixture { path: PathBuf },
    Process { command: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProjectProfile {
    pub graph_path: PathBuf,
    pub cluster_paths: Vec<PathBuf>,
    pub ingress: ResolvedProjectIngress,
    pub adapter_path: Option<PathBuf>,
    pub egress_config_path: Option<PathBuf>,
    pub capture_output: Option<PathBuf>,
    pub pretty_capture: bool,
    pub max_duration: Option<Duration>,
    pub max_events: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub root: PathBuf,
    pub manifest: ProjectManifest,
}

impl ResolvedProject {
    pub fn profile_names(&self) -> Vec<String> {
        self.manifest.profiles.keys().cloned().collect()
    }

    pub fn resolve_run_profile(&self, name: &str) -> Result<ResolvedProjectProfile, ProjectError> {
        let profile =
            self.manifest
                .profiles
                .get(name)
                .ok_or_else(|| ProjectError::ProfileNotFound {
                    name: name.to_string(),
                })?;

        let ingress_count =
            usize::from(profile.fixture.is_some()) + usize::from(profile.ingress.is_some());
        // Project profiles must resolve to exactly one driver source so downstream host/SDK
        // prep never has to guess between fixture-backed and process-backed ingress.
        if ingress_count == 0 {
            return Err(ProjectError::ProfileInvalid {
                name: name.to_string(),
                detail: "profile must declare exactly one ingress source (fixture or ingress)"
                    .to_string(),
            });
        }
        if ingress_count > 1 {
            return Err(ProjectError::ProfileInvalid {
                name: name.to_string(),
                detail: "fixture and ingress are mutually exclusive".to_string(),
            });
        }

        let ingress = if let Some(fixture) = &profile.fixture {
            ResolvedProjectIngress::Fixture {
                path: self.root.join(fixture),
            }
        } else {
            let Some(ProjectIngress::Process { command }) = &profile.ingress else {
                return Err(ProjectError::ProfileInvalid {
                    name: name.to_string(),
                    detail: "unsupported ingress configuration".to_string(),
                });
            };
            if command.is_empty() {
                return Err(ProjectError::ProfileInvalid {
                    name: name.to_string(),
                    detail: "process ingress command must not be empty".to_string(),
                });
            }
            ResolvedProjectIngress::Process {
                command: command.clone(),
            }
        };

        Ok(ResolvedProjectProfile {
            graph_path: self.root.join(&profile.graph),
            // Project resolution always adds the conventional `clusters/` directory; discovery
            // decides later whether anything actually exists there.
            cluster_paths: vec![self.root.join("clusters")],
            ingress,
            adapter_path: profile.adapter.as_ref().map(|path| self.root.join(path)),
            egress_config_path: profile.egress.as_ref().map(|path| self.root.join(path)),
            capture_output: profile
                .capture_output
                .as_ref()
                .map(|path| self.root.join(path)),
            pretty_capture: profile.pretty_capture.unwrap_or(false),
            max_duration: profile.max_duration,
            max_events: profile.max_events,
        })
    }
}

mod duration_option_serde {
    use std::time::Duration;

    use ergo_prod_duration::parse_duration_literal;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Option::<String>::deserialize(deserializer)?;
        raw.map(|value| parse_duration_literal(&value))
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

pub fn load_project(start: &Path) -> Result<ResolvedProject, ProjectError> {
    let root = discover_project_root(start)?;
    let manifest_path = root.join("ergo.toml");
    let raw = fs::read_to_string(&manifest_path).map_err(|err| ProjectError::ProjectRead {
        path: manifest_path.clone(),
        detail: err.to_string(),
    })?;
    let manifest =
        toml::from_str::<ProjectManifest>(&raw).map_err(|err| ProjectError::ProjectParse {
            path: manifest_path,
            detail: err.to_string(),
        })?;
    Ok(ResolvedProject { root, manifest })
}

pub fn discover_project_root(start: &Path) -> Result<PathBuf, ProjectError> {
    if start.is_file() && start.file_name().is_some_and(|name| name == "ergo.toml") {
        return Ok(start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")));
    }

    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let original = start.to_path_buf();

    loop {
        if current.join("ergo.toml").exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(ProjectError::ProjectRootNotFound { start: original });
        }
    }
}

#[cfg(test)]
mod tests;
