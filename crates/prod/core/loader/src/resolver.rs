use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::discovery::InMemorySourceInput;
use crate::io::{canonicalize_or_self, LoaderDiscoveryError, LoaderError, LoaderIoError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct LogicalPath {
    segments: Vec<String>,
}

impl LogicalPath {
    fn parse_source_id(input: &str) -> Result<Self, LoaderError> {
        Self::parse(input, "in-memory source_id")
    }

    fn parse_search_root(input: &str) -> Result<Self, LoaderError> {
        Self::parse(input, "in-memory search_root")
    }

    fn parse(input: &str, field_name: &str) -> Result<Self, LoaderError> {
        if input.is_empty() {
            return Err(discovery_error(format!("{field_name} must not be empty")));
        }
        if input.starts_with('/') {
            return Err(discovery_error(format!(
                "{field_name} must be a relative logical path"
            )));
        }
        if input.contains('\\') {
            return Err(discovery_error(format!(
                "{field_name} must use '/' separators"
            )));
        }
        if input.contains(':') {
            return Err(discovery_error(format!(
                "{field_name} must not contain ':'"
            )));
        }

        let mut segments = Vec::new();
        for segment in input.split('/') {
            if segment.is_empty() {
                return Err(discovery_error(format!(
                    "{field_name} must not contain empty path segments"
                )));
            }
            if segment == "." || segment == ".." {
                return Err(discovery_error(format!(
                    "{field_name} must not contain '.' or '..' segments"
                )));
            }
            segments.push(segment.to_string());
        }

        Ok(Self { segments })
    }

    fn root_dir() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    fn parent_dir(&self) -> Self {
        if self.segments.len() <= 1 {
            Self::root_dir()
        } else {
            Self {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
            }
        }
    }

    fn join_file(&self, filename: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(filename.to_string());
        Self { segments }
    }

    fn join_clusters_file(&self, filename: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push("clusters".to_string());
        segments.push(filename.to_string());
        Self { segments }
    }

    fn file_name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    fn display(&self) -> String {
        if self.segments.is_empty() {
            ".".to_string()
        } else {
            self.segments.join("/")
        }
    }

    fn to_source_id(&self) -> String {
        self.segments.join("/")
    }
}

pub(crate) fn normalize_in_memory_source_id(source_id: &str) -> Result<String, LoaderError> {
    LogicalPath::parse_source_id(source_id).map(|path| path.to_source_id())
}

#[derive(Debug, Clone)]
pub(crate) enum SourceRef {
    Filesystem {
        canonical_path: PathBuf,
        lexical_path: PathBuf,
    },
    InMemory {
        source_id: String,
        logical_path: LogicalPath,
        source_label: String,
    },
}

impl SourceRef {
    pub(crate) fn from_opened_path(path: &Path) -> Self {
        Self::Filesystem {
            canonical_path: canonicalize_or_self(path),
            lexical_path: path.to_path_buf(),
        }
    }

    pub(crate) fn from_in_memory(
        source_id: String,
        logical_path: LogicalPath,
        source_label: &str,
    ) -> Self {
        Self::InMemory {
            source_id,
            logical_path,
            source_label: source_label.to_string(),
        }
    }

    pub(crate) fn opened_label(&self) -> String {
        match self {
            Self::Filesystem { lexical_path, .. } => lexical_path.display().to_string(),
            Self::InMemory { source_label, .. } => source_label.clone(),
        }
    }

    pub(crate) fn filesystem_canonical_path(&self) -> Option<&Path> {
        match self {
            Self::Filesystem { canonical_path, .. } => Some(canonical_path),
            Self::InMemory { .. } => None,
        }
    }

    pub(crate) fn in_memory_source_id(&self) -> Option<&str> {
        match self {
            Self::InMemory { source_id, .. } => Some(source_id),
            Self::Filesystem { .. } => None,
        }
    }

    fn identity_kind(&self) -> IdentityKind<'_> {
        match self {
            Self::Filesystem { canonical_path, .. } => IdentityKind::Filesystem(canonical_path),
            Self::InMemory { source_id, .. } => IdentityKind::InMemory(source_id),
        }
    }
}

enum IdentityKind<'a> {
    Filesystem(&'a PathBuf),
    InMemory(&'a str),
}

impl PartialEq for SourceRef {
    fn eq(&self, other: &Self) -> bool {
        match (self.identity_kind(), other.identity_kind()) {
            (IdentityKind::Filesystem(left), IdentityKind::Filesystem(right)) => left == right,
            (IdentityKind::InMemory(left), IdentityKind::InMemory(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for SourceRef {}

impl Hash for SourceRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.identity_kind() {
            IdentityKind::Filesystem(path) => {
                0u8.hash(state);
                path.hash(state);
            }
            IdentityKind::InMemory(source_id) => {
                1u8.hash(state);
                source_id.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSourceCandidate {
    pub(crate) source_ref: SourceRef,
    pub(crate) opened_label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolverResult {
    pub(crate) found: Vec<ResolvedSourceCandidate>,
    pub(crate) search_trace: Vec<String>,
}

pub(crate) trait ClusterResolver {
    fn resolve(
        &self,
        cluster_id: &str,
        referring_source: Option<&SourceRef>,
    ) -> Result<ResolverResult, LoaderError>;

    fn read(&self, source_ref: &SourceRef) -> Result<String, LoaderError>;
}

#[derive(Debug, Clone)]
pub(crate) struct FilesystemResolver {
    search_paths: Vec<PathBuf>,
}

impl FilesystemResolver {
    pub(crate) fn new(search_paths: &[PathBuf]) -> Self {
        Self {
            search_paths: search_paths.to_vec(),
        }
    }

    pub(crate) fn resolve_existing_candidate_paths(
        &self,
        base_dir: &Path,
        cluster_id: &str,
    ) -> Vec<PathBuf> {
        collect_candidate_search(base_dir, cluster_id, &self.search_paths)
            .existing_sources
            .into_iter()
            .filter_map(|source| match source {
                SourceRef::Filesystem { lexical_path, .. } => Some(lexical_path),
                SourceRef::InMemory { .. } => None,
            })
            .collect()
    }
}

impl ClusterResolver for FilesystemResolver {
    fn resolve(
        &self,
        cluster_id: &str,
        referring_source: Option<&SourceRef>,
    ) -> Result<ResolverResult, LoaderError> {
        let base_dir = match referring_source {
            Some(SourceRef::Filesystem { lexical_path, .. }) => {
                lexical_path.parent().unwrap_or_else(|| Path::new("."))
            }
            Some(SourceRef::InMemory { .. }) => {
                return Err(discovery_error(
                    "filesystem resolver received in-memory referrer".to_string(),
                ))
            }
            None => Path::new("."),
        };
        let search = collect_candidate_search(base_dir, cluster_id, &self.search_paths);

        Ok(ResolverResult {
            found: search
                .existing_sources
                .into_iter()
                .map(|source_ref| ResolvedSourceCandidate {
                    opened_label: source_ref.opened_label(),
                    source_ref,
                })
                .collect(),
            search_trace: search
                .searched_paths
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
        })
    }

    fn read(&self, source_ref: &SourceRef) -> Result<String, LoaderError> {
        let SourceRef::Filesystem { lexical_path, .. } = source_ref else {
            return Err(discovery_error(
                "filesystem resolver received non-filesystem source".to_string(),
            ));
        };
        fs::read_to_string(lexical_path).map_err(|err| {
            LoaderError::Io(LoaderIoError {
                path: lexical_path.clone(),
                message: format!("read graph '{}': {err}", lexical_path.display()),
            })
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InMemoryResolver {
    ordered_source_ids: Vec<String>,
    search_roots: Vec<LogicalPath>,
    sources_by_id: HashMap<String, InMemorySourceRecord>,
}

#[derive(Debug, Clone)]
struct InMemorySourceRecord {
    source_ref: SourceRef,
    content: String,
}

impl InMemoryResolver {
    pub(crate) fn new(
        sources: &[InMemorySourceInput],
        search_roots: &[String],
    ) -> Result<Self, LoaderError> {
        let mut ordered_source_ids = Vec::new();
        let mut sources_by_id = HashMap::new();
        let mut seen_ids = HashSet::new();
        let mut seen_labels = HashSet::new();

        for source in sources {
            let logical_path = LogicalPath::parse_source_id(&source.source_id)?;
            let normalized_source_id = logical_path.to_source_id();
            if source.source_label.is_empty() {
                return Err(discovery_error(
                    "in-memory source_label must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(normalized_source_id.clone()) {
                return Err(discovery_error(format!(
                    "duplicate in-memory source_id '{}'",
                    normalized_source_id
                )));
            }
            if !seen_labels.insert(source.source_label.clone()) {
                return Err(discovery_error(format!(
                    "duplicate in-memory source_label '{}' (tranche 1 requires unique diagnostic labels per call)",
                    source.source_label
                )));
            }

            ordered_source_ids.push(normalized_source_id.clone());
            sources_by_id.insert(
                normalized_source_id.clone(),
                InMemorySourceRecord {
                    source_ref: SourceRef::from_in_memory(
                        normalized_source_id,
                        logical_path,
                        &source.source_label,
                    ),
                    content: source.content.clone(),
                },
            );
        }

        Ok(Self {
            ordered_source_ids,
            search_roots: search_roots
                .iter()
                .map(|root| LogicalPath::parse_search_root(root))
                .collect::<Result<Vec<_>, _>>()?,
            sources_by_id,
        })
    }

    pub(crate) fn root_source(&self, root_source_id: &str) -> Result<SourceRef, LoaderError> {
        let normalized_root_source_id = normalize_in_memory_source_id(root_source_id)?;
        self.sources_by_id
            .get(&normalized_root_source_id)
            .map(|source| source.source_ref.clone())
            .ok_or_else(|| {
                discovery_error(format!(
                    "root in-memory source_id '{}' was not provided",
                    normalized_root_source_id
                ))
            })
    }
}

impl ClusterResolver for InMemoryResolver {
    fn resolve(
        &self,
        cluster_id: &str,
        referring_source: Option<&SourceRef>,
    ) -> Result<ResolverResult, LoaderError> {
        let base_dir = match referring_source {
            Some(SourceRef::InMemory { logical_path, .. }) => logical_path.parent_dir(),
            Some(SourceRef::Filesystem { .. }) => {
                return Err(discovery_error(
                    "in-memory resolver received filesystem referrer".to_string(),
                ))
            }
            None => LogicalPath::root_dir(),
        };
        let candidate_paths =
            collect_in_memory_candidate_paths(&base_dir, cluster_id, &self.search_roots);
        let candidate_set: HashSet<LogicalPath> = candidate_paths.iter().cloned().collect();

        let mut found = Vec::new();
        for source_id in &self.ordered_source_ids {
            let Some(source) = self.sources_by_id.get(source_id) else {
                continue;
            };
            let SourceRef::InMemory {
                logical_path,
                source_label,
                ..
            } = &source.source_ref
            else {
                continue;
            };
            if candidate_set.contains(logical_path) {
                found.push(ResolvedSourceCandidate {
                    source_ref: source.source_ref.clone(),
                    opened_label: source_label.clone(),
                });
            }
        }

        Ok(ResolverResult {
            found,
            search_trace: candidate_paths
                .into_iter()
                .map(|path| path.display())
                .collect(),
        })
    }

    fn read(&self, source_ref: &SourceRef) -> Result<String, LoaderError> {
        let Some(source_id) = source_ref.in_memory_source_id() else {
            return Err(discovery_error(
                "in-memory resolver received non-in-memory source".to_string(),
            ));
        };
        self.sources_by_id
            .get(source_id)
            .map(|source| source.content.clone())
            .ok_or_else(|| {
                discovery_error(format!(
                    "in-memory source_id '{}' was not provided",
                    source_id
                ))
            })
    }
}

struct CandidateSearch {
    searched_paths: Vec<PathBuf>,
    existing_sources: Vec<SourceRef>,
}

fn collect_candidate_search(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> CandidateSearch {
    let candidate_paths = collect_candidate_paths(base_dir, cluster_id, search_paths);

    let mut existing_seen = HashSet::new();
    let mut existing_sources = Vec::new();
    for candidate in &candidate_paths {
        if !candidate.is_file() {
            continue;
        }
        let source_ref = SourceRef::from_opened_path(candidate);
        if existing_seen.insert(source_ref.clone()) {
            existing_sources.push(source_ref);
        }
    }

    CandidateSearch {
        searched_paths: candidate_paths,
        existing_sources,
    }
}

fn collect_candidate_paths(
    base_dir: &Path,
    cluster_id: &str,
    search_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join(&filename),
        base_dir.join("clusters").join(&filename),
    ];

    for path in search_paths {
        candidates.push(path.join(&filename));
        if path.file_name() != Some(OsStr::new("clusters")) {
            candidates.push(path.join("clusters").join(&filename));
        }
    }

    let mut searched_seen = HashSet::new();
    let mut searched_paths = Vec::new();
    for candidate in candidates {
        if searched_seen.insert(candidate.clone()) {
            searched_paths.push(candidate);
        }
    }

    searched_paths
}

fn collect_in_memory_candidate_paths(
    base_dir: &LogicalPath,
    cluster_id: &str,
    search_roots: &[LogicalPath],
) -> Vec<LogicalPath> {
    let filename = format!("{cluster_id}.yaml");

    let mut candidates = vec![
        base_dir.join_file(&filename),
        base_dir.join_clusters_file(&filename),
    ];

    for path in search_roots {
        candidates.push(path.join_file(&filename));
        if path.file_name() != Some("clusters") {
            candidates.push(path.join_clusters_file(&filename));
        }
    }

    let mut searched_seen = HashSet::new();
    let mut searched_paths = Vec::new();
    for candidate in candidates {
        if searched_seen.insert(candidate.clone()) {
            searched_paths.push(candidate);
        }
    }

    searched_paths
}

fn discovery_error(message: String) -> LoaderError {
    LoaderError::Discovery(LoaderDiscoveryError { message })
}
