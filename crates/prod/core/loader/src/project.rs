use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
            cluster_paths: vec![self.root.join("clusters")],
            ingress,
            adapter_path: profile.adapter.as_ref().map(|path| self.root.join(path)),
            egress_config_path: profile.egress.as_ref().map(|path| self.root.join(path)),
            capture_output: profile
                .capture_output
                .as_ref()
                .map(|path| self.root.join(path)),
            pretty_capture: profile.pretty_capture.unwrap_or(false),
        })
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_temp_dir(label: &str) -> PathBuf {
        let index = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ergo_loader_project_{label}_{}_{}_{}",
            std::process::id(),
            index,
            nanos
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(base: &Path, rel: &str, contents: &str) -> PathBuf {
        let path = base.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, contents).expect("write file");
        path
    }

    #[test]
    fn discovers_project_root_from_nested_path() {
        let root = make_temp_dir("discover");
        write_file(
            &root,
            "ergo.toml",
            "name = \"sdk-project\"\nversion = \"0.1.0\"\n",
        );
        let nested = root.join("graphs/deep/path");
        fs::create_dir_all(&nested).expect("create nested");

        let discovered = discover_project_root(&nested).expect("discover project");
        assert_eq!(discovered, root);

        let _ = fs::remove_dir_all(discovered);
    }

    #[test]
    fn resolve_run_profile_adds_clusters_path_and_fixture_ingress() {
        let root = make_temp_dir("resolve");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.backtest]
graph = "graphs/strategy.yaml"
fixture = "fixtures/backtest.jsonl"
capture_output = "captures/backtest.capture.json"
"#,
        );

        let project = load_project(&root).expect("load project");
        let resolved = project
            .resolve_run_profile("backtest")
            .expect("resolve profile");

        assert_eq!(resolved.graph_path, root.join("graphs/strategy.yaml"));
        assert_eq!(resolved.cluster_paths, vec![root.join("clusters")]);
        assert_eq!(
            resolved.capture_output,
            Some(root.join("captures/backtest.capture.json"))
        );
        assert_eq!(
            resolved.ingress,
            ResolvedProjectIngress::Fixture {
                path: root.join("fixtures/backtest.jsonl")
            }
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_run_profile_rejects_mutually_exclusive_ingress_sources() {
        let root = make_temp_dir("invalid_ingress");
        write_file(
            &root,
            "ergo.toml",
            r#"
name = "sdk-project"
version = "0.1.0"

[profiles.live]
graph = "graphs/strategy.yaml"
fixture = "fixtures/live.jsonl"

[profiles.live.ingress]
type = "process"
command = ["python3", "channels/ingress/live.py"]
"#,
        );

        let project = load_project(&root).expect("load project");
        let err = project
            .resolve_run_profile("live")
            .expect_err("mutually exclusive ingress must fail");
        assert!(matches!(err, ProjectError::ProfileInvalid { .. }));

        let _ = fs::remove_dir_all(root);
    }
}
