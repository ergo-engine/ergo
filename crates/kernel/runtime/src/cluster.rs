use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::common::{doc_anchor_for_rule, ErrorInfo, Phase};
use semver::{Version as SemverVersion, VersionReq};

pub type Version = String;
pub type NodeId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterDefinition {
    pub id: String,
    pub version: Version,
    pub nodes: HashMap<NodeId, NodeInstance>,
    pub edges: Vec<Edge>,
    pub input_ports: Vec<InputPortSpec>,
    pub output_ports: Vec<OutputPortSpec>,
    pub parameters: Vec<ParameterSpec>,
    pub declared_signature: Option<Signature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeInstance {
    pub id: NodeId,
    pub kind: NodeKind,
    pub parameter_bindings: HashMap<String, ParameterBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Impl {
        impl_id: String,
        version: Version,
    },
    Cluster {
        cluster_id: String,
        version: Version,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: OutputRef,
    pub to: InputRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputRef {
    pub node_id: NodeId,
    pub port_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputRef {
    pub node_id: NodeId,
    pub port_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputPortSpec {
    pub name: String,
    pub maps_to: GraphInputPlaceholder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputPortSpec {
    pub name: String,
    pub maps_to: OutputRef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphInputPlaceholder {
    pub name: String,
    pub ty: ValueType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterDefault {
    Literal(ParameterValue),
    DeriveKey { slot_name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub name: String,
    pub ty: ParameterType,
    pub default: Option<ParameterDefault>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterBinding {
    Literal { value: ParameterValue },
    Exposed { parent_param: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub kind: BoundaryKind,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub has_side_effects: bool,
    pub is_origin: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortSpec {
    pub name: String,
    pub ty: ValueType,
    pub cardinality: Cardinality,
    pub wireable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryKind {
    SourceLike,
    ComputeLike,
    TriggerLike,
    ActionLike,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Number,
    Series,
    Bool,
    Event,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    Single,
    Multiple,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Int,
    Number,
    Bool,
    String,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Int(i64),
    Number(f64),
    Bool(bool),
    String(String),
    Enum(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveKind {
    Source,
    Compute,
    Trigger,
    Action,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputMetadata {
    pub value_type: ValueType,
    pub cardinality: Cardinality,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveMetadata {
    pub kind: PrimitiveKind,
    pub inputs: Vec<InputMetadata>,
    pub outputs: HashMap<String, OutputMetadata>,
    /// A.1: Parameter specs with defaults for expansion-time resolution.
    pub parameters: Vec<ParameterMetadata>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputMetadata {
    pub name: String,
    pub value_type: ValueType,
    pub required: bool,
}

/// A.1: Parameter metadata for primitives, including defaults.
/// Used during expansion to resolve parameters when no binding is provided.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterMetadata {
    pub name: String,
    pub ty: ParameterType,
    pub default: Option<ParameterValue>,
    pub required: bool,
}

/// Expansion output. Contains only topology, primitive identity, and authoring trace.
/// `boundary_inputs` and `boundary_outputs` are retained for signature inference only
/// and must not influence runtime execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedGraph {
    pub nodes: HashMap<String, ExpandedNode>,
    pub edges: Vec<ExpandedEdge>,
    pub boundary_inputs: Vec<InputPortSpec>,
    pub boundary_outputs: Vec<OutputPortSpec>,
}

/// X.9 enforcement: Clusters compile away here.
///
/// `ExpandedNode` holds only `ImplementationInstance` — no `NodeKind` enum.
/// The type system guarantees authoring constructs cannot reach execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedNode {
    pub runtime_id: String,
    pub authoring_path: Vec<(String, NodeId)>,
    pub implementation: ImplementationInstance,
    pub parameters: HashMap<String, ParameterValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplementationInstance {
    // Identity-only; no semantic or configuration fields.
    pub impl_id: String,
    /// Authoring selector as written in the graph (exact semver or constraint).
    pub requested_version: Version,
    /// Resolved concrete semver used for expansion/runtime.
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedEdge {
    pub from: ExpandedEndpoint,
    pub to: ExpandedEndpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpandedEndpoint {
    NodePort { node_id: String, port_name: String },
    ExternalInput { name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpandError {
    /// A kernel expansion invariant was violated.
    InvariantViolation(String),
    EmptyCluster,
    MissingCluster {
        id: String,
        version: Version,
    },
    /// I.6: Node version selector is not valid semver or semver constraint syntax.
    InvalidVersionSelector {
        target_kind: VersionTargetKind,
        id: String,
        selector: Version,
    },
    /// I.6: No available version satisfies the selector.
    UnsatisfiedVersionConstraint {
        target_kind: VersionTargetKind,
        id: String,
        selector: Version,
        available_versions: Vec<Version>,
    },
    /// I.6: Registered available version is not strict semver.
    InvalidAvailableVersion {
        target_kind: VersionTargetKind,
        id: String,
        version: Version,
    },
    DuplicateInputPort {
        name: String,
    },
    DuplicateOutputPort {
        name: String,
    },
    DuplicateParameter {
        name: String,
    },
    ParameterDefaultTypeMismatch {
        name: String,
        expected: ParameterType,
        got: ParameterType,
    },
    InvalidDeriveKeySlot {
        parameter: String,
    },
    SignatureInferenceFailed(SignatureInferenceError),
    DeclaredSignatureInvalid(ClusterValidationError),
    /// I.3: Required parameter has no binding and no default
    MissingRequiredParameter {
        cluster_id: String,
        parameter: String,
    },
    /// I.4: Literal binding has wrong type
    ParameterBindingTypeMismatch {
        cluster_id: String,
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    /// I.5: Exposed binding references nonexistent parent parameter
    ExposedParameterNotFound {
        cluster_id: String,
        parameter: String,
        referenced: String,
    },
    /// I.4: Exposed binding has incompatible type with parent parameter
    ExposedParameterTypeMismatch {
        cluster_id: String,
        parameter: String,
        expected: ParameterType,
        got: ParameterType,
    },
    /// Exposed binding was not resolved during expansion (no parent provided a value)
    UnresolvedExposedBinding {
        node_id: String,
        parameter: String,
        referenced: String,
    },
    /// I.7: Binding references a parameter not declared in the target's manifest
    UndeclaredParameter {
        node_id: String,
        parameter: String,
    },
    /// D.4: Boundary output references unmapped node_id
    UnmappedBoundaryOutput {
        port_name: String,
        node_id: String,
    },
    /// D.4: Nested cluster output port references unmapped node
    UnmappedNestedOutput {
        cluster_id: String,
        port_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SignatureInferenceError {
    MissingPrimitive {
        id: String,
        version: Version,
    },
    MissingNode(String),
    MissingOutput {
        impl_id: String,
        version: Version,
        output: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionTargetKind {
    Primitive,
    Cluster,
}

impl VersionTargetKind {
    fn label(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::Cluster => "cluster",
        }
    }
}

/// D.11: Errors arising from declared signature validation
#[derive(Debug, Clone, PartialEq)]
pub enum ClusterValidationError {
    /// Declared wireability exceeds inferred wireability (D.11 violation)
    WireabilityExceedsInferred { port_name: String },
}

impl ErrorInfo for SignatureInferenceError {
    fn rule_id(&self) -> &'static str {
        "D.4"
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        doc_anchor_for_rule(self.rule_id())
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::MissingPrimitive { id, version } => {
                Cow::Owned(format!("Missing primitive '{}@{}'", id, version))
            }
            Self::MissingNode(node) => Cow::Owned(format!("Missing node '{}'", node)),
            Self::MissingOutput {
                impl_id,
                version,
                output,
            } => Cow::Owned(format!(
                "Missing output '{}' on primitive '{}@{}'",
                output, impl_id, version
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed("$.output_ports"))
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(
            "Ensure all output ports map to existing node outputs",
        ))
    }
}

impl ErrorInfo for ClusterValidationError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::WireabilityExceedsInferred { .. } => "D.11",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        doc_anchor_for_rule(self.rule_id())
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::WireabilityExceedsInferred { port_name } => Cow::Owned(format!(
                "Declared wireability exceeds inferred for port '{}'",
                port_name
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed("$.declared_signature"))
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(
            "Adjust declared wireability to be <= inferred wireability",
        ))
    }
}

impl ErrorInfo for ExpandError {
    fn rule_id(&self) -> &'static str {
        match self {
            Self::InvariantViolation(_) => "E.3",
            Self::EmptyCluster => "D.1",
            Self::DuplicateInputPort { .. } => "D.5",
            Self::DuplicateOutputPort { .. } => "D.6",
            Self::DuplicateParameter { .. } => "D.9",
            Self::ParameterDefaultTypeMismatch { .. } => "D.8",
            Self::InvalidDeriveKeySlot { .. } => "D.8",
            Self::SignatureInferenceFailed(_) => "D.4",
            Self::DeclaredSignatureInvalid(_) => "D.10",
            Self::MissingCluster { .. } => "E.9",
            Self::InvalidVersionSelector { .. }
            | Self::UnsatisfiedVersionConstraint { .. }
            | Self::InvalidAvailableVersion { .. } => "I.6",
            Self::MissingRequiredParameter { .. } | Self::UnresolvedExposedBinding { .. } => "I.3",
            Self::ParameterBindingTypeMismatch { .. }
            | Self::ExposedParameterTypeMismatch { .. } => "I.4",
            Self::ExposedParameterNotFound { .. } => "I.5",
            Self::UndeclaredParameter { .. } => "I.7",
            Self::UnmappedBoundaryOutput { .. } | Self::UnmappedNestedOutput { .. } => "D.4",
        }
    }

    fn phase(&self) -> Phase {
        Phase::Composition
    }

    fn doc_anchor(&self) -> &'static str {
        doc_anchor_for_rule(self.rule_id())
    }

    fn summary(&self) -> Cow<'static, str> {
        match self {
            Self::InvariantViolation(msg) => Cow::Owned(msg.clone()),
            Self::EmptyCluster => Cow::Borrowed("Cluster contains no nodes"),
            Self::MissingCluster { id, version } => {
                Cow::Owned(format!("Missing cluster '{}@{}'", id, version))
            }
            Self::InvalidVersionSelector {
                target_kind,
                id,
                selector,
            } => Cow::Owned(format!(
                "Invalid {} version selector '{}@{}' (expected exact semver or semver constraint)",
                target_kind.label(),
                id,
                selector
            )),
            Self::UnsatisfiedVersionConstraint {
                target_kind,
                id,
                selector,
                available_versions,
            } => Cow::Owned(format!(
                "No available {} version for '{}' satisfies selector '{}' (available: {})",
                target_kind.label(),
                id,
                selector,
                if available_versions.is_empty() {
                    "<none>".to_string()
                } else {
                    available_versions.join(", ")
                }
            )),
            Self::InvalidAvailableVersion {
                target_kind,
                id,
                version,
            } => Cow::Owned(format!(
                "Registered {} version '{}@{}' is not valid semver",
                target_kind.label(),
                id,
                version
            )),
            Self::DuplicateInputPort { name } => {
                Cow::Owned(format!("Duplicate input port name: '{}'", name))
            }
            Self::DuplicateOutputPort { name } => {
                Cow::Owned(format!("Duplicate output port name: '{}'", name))
            }
            Self::DuplicateParameter { name } => {
                Cow::Owned(format!("Duplicate parameter name: '{}'", name))
            }
            Self::ParameterDefaultTypeMismatch {
                name,
                expected,
                got,
            } => Cow::Owned(format!(
                "Parameter '{}' default has wrong type (expected {:?}, got {:?})",
                name, expected, got
            )),
            Self::InvalidDeriveKeySlot { parameter } => Cow::Owned(format!(
                "Parameter '{}' has derive_key default with empty slot_name",
                parameter
            )),
            Self::SignatureInferenceFailed(inner) => inner.summary(),
            Self::DeclaredSignatureInvalid(inner) => inner.summary(),
            Self::MissingRequiredParameter {
                cluster_id,
                parameter,
            } => Cow::Owned(format!(
                "Missing required parameter '{}' for cluster '{}'",
                parameter, cluster_id
            )),
            Self::ParameterBindingTypeMismatch {
                cluster_id,
                parameter,
                expected,
                got,
            } => Cow::Owned(format!(
                "Parameter '{}' on cluster '{}' has wrong type (expected {:?}, got {:?})",
                parameter, cluster_id, expected, got
            )),
            Self::ExposedParameterNotFound {
                cluster_id,
                parameter,
                referenced,
            } => Cow::Owned(format!(
                "Exposed parameter '{}' on cluster '{}' references missing '{}'",
                parameter, cluster_id, referenced
            )),
            Self::ExposedParameterTypeMismatch {
                cluster_id,
                parameter,
                expected,
                got,
            } => Cow::Owned(format!(
                "Exposed parameter '{}' on cluster '{}' has wrong type (expected {:?}, got {:?})",
                parameter, cluster_id, expected, got
            )),
            Self::UnresolvedExposedBinding {
                node_id,
                parameter,
                referenced,
            } => Cow::Owned(format!(
                "Unresolved exposed binding '{}' for parameter '{}' on node '{}'",
                referenced, parameter, node_id
            )),
            Self::UndeclaredParameter { node_id, parameter } => Cow::Owned(format!(
                "Undeclared parameter '{}' on node '{}' (not in manifest)",
                parameter, node_id
            )),
            Self::UnmappedBoundaryOutput { port_name, .. } => Cow::Owned(format!(
                "Boundary output '{}' maps to a missing node output",
                port_name
            )),
            Self::UnmappedNestedOutput {
                cluster_id,
                port_name,
            } => Cow::Owned(format!(
                "Nested output '{}' in cluster '{}' maps to a missing node output",
                port_name, cluster_id
            )),
        }
    }

    fn path(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvariantViolation(_) => Some(Cow::Borrowed("$.edges")),
            Self::EmptyCluster => Some(Cow::Borrowed("$.nodes")),
            Self::DuplicateInputPort { .. } => Some(Cow::Borrowed("$.input_ports")),
            Self::DuplicateOutputPort { .. } => Some(Cow::Borrowed("$.output_ports")),
            Self::DuplicateParameter { .. } => Some(Cow::Borrowed("$.parameters")),
            Self::ParameterDefaultTypeMismatch { .. } => Some(Cow::Borrowed("$.parameters")),
            Self::InvalidDeriveKeySlot { .. } => Some(Cow::Borrowed("$.parameters")),
            Self::SignatureInferenceFailed(_) => Some(Cow::Borrowed("$.output_ports")),
            Self::DeclaredSignatureInvalid(_) => Some(Cow::Borrowed("$.declared_signature")),
            Self::InvalidVersionSelector { .. }
            | Self::UnsatisfiedVersionConstraint { .. }
            | Self::InvalidAvailableVersion { .. } => Some(Cow::Borrowed("$.nodes")),
            Self::MissingRequiredParameter { .. }
            | Self::ParameterBindingTypeMismatch { .. }
            | Self::ExposedParameterNotFound { .. }
            | Self::ExposedParameterTypeMismatch { .. }
            | Self::UnresolvedExposedBinding { .. }
            | Self::UndeclaredParameter { .. } => Some(Cow::Borrowed("$.nodes")),
            Self::UnmappedBoundaryOutput { .. } => Some(Cow::Borrowed("$.output_ports")),
            Self::UnmappedNestedOutput { .. } => Some(Cow::Borrowed("$.nodes")),
            Self::MissingCluster { .. } => Some(Cow::Borrowed("$.nodes")),
        }
    }

    fn fix(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::InvariantViolation(_) => None,
            Self::EmptyCluster => Some(Cow::Borrowed("Add at least one node to the cluster")),
            Self::MissingCluster { .. } => Some(Cow::Borrowed(
                "Ensure referenced cluster ID and version exist",
            )),
            Self::InvalidVersionSelector { .. } => Some(Cow::Borrowed(
                "Use strict semver (e.g. '1.2.3') or a semver constraint (e.g. '^1.2')",
            )),
            Self::UnsatisfiedVersionConstraint { .. } => Some(Cow::Borrowed(
                "Publish or reference a version that satisfies the selector",
            )),
            Self::InvalidAvailableVersion { .. } => Some(Cow::Borrowed(
                "Register only strict semver versions in the catalog/cluster loader",
            )),
            Self::DuplicateInputPort { name } => Some(Cow::Owned(format!(
                "Rename input port '{}' to a unique name",
                name
            ))),
            Self::DuplicateOutputPort { name } => Some(Cow::Owned(format!(
                "Rename output port '{}' to a unique name",
                name
            ))),
            Self::DuplicateParameter { name } => Some(Cow::Owned(format!(
                "Rename parameter '{}' to a unique name",
                name
            ))),
            Self::ParameterDefaultTypeMismatch { name, expected, .. } => Some(Cow::Owned(format!(
                "Set default for '{}' to type {:?}",
                name, expected
            ))),
            Self::InvalidDeriveKeySlot { parameter } => Some(Cow::Owned(format!(
                "Provide a non-empty slot_name for derive_key on parameter '{}'",
                parameter
            ))),
            Self::SignatureInferenceFailed(_) => Some(Cow::Borrowed(
                "Ensure output ports map to valid node outputs",
            )),
            Self::DeclaredSignatureInvalid(_) => Some(Cow::Borrowed(
                "Align declared signature with the inferred signature",
            )),
            Self::MissingRequiredParameter { parameter, .. } => Some(Cow::Owned(format!(
                "Bind required parameter '{}' or provide a default",
                parameter
            ))),
            Self::ParameterBindingTypeMismatch {
                parameter,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Bind parameter '{}' with type {:?}",
                parameter, expected
            ))),
            Self::ExposedParameterNotFound { referenced, .. } => Some(Cow::Owned(format!(
                "Expose an existing parent parameter '{}'",
                referenced
            ))),
            Self::ExposedParameterTypeMismatch {
                parameter,
                expected,
                ..
            } => Some(Cow::Owned(format!(
                "Match exposed parameter '{}' type to {:?}",
                parameter, expected
            ))),
            Self::UnresolvedExposedBinding { referenced, .. } => Some(Cow::Owned(format!(
                "Provide a value for exposed parameter '{}'",
                referenced
            ))),
            Self::UndeclaredParameter { parameter, .. } => Some(Cow::Owned(format!(
                "Remove binding '{}' or add it to the primitive's manifest parameters",
                parameter
            ))),
            Self::UnmappedBoundaryOutput { port_name, .. } => Some(Cow::Owned(format!(
                "Map output port '{}' to a valid node output",
                port_name
            ))),
            Self::UnmappedNestedOutput { port_name, .. } => Some(Cow::Owned(format!(
                "Map nested output '{}' to a valid node output",
                port_name
            ))),
        }
    }
}

pub trait ClusterLoader {
    fn load(&self, id: &str, version: &Version) -> Option<ClusterDefinition>;
}

pub trait ClusterVersionIndex {
    fn available_versions(&self, id: &str) -> Vec<Version>;
}

pub trait PrimitiveCatalog {
    fn get(&self, id: &str, version: &Version) -> Option<PrimitiveMetadata>;
}

pub trait PrimitiveVersionIndex {
    fn available_versions(&self, id: &str) -> Vec<Version>;
}

pub fn expand<L, C>(
    cluster_def: &ClusterDefinition,
    loader: &L,
    catalog: &C,
) -> Result<ExpandedGraph, ExpandError>
where
    L: ClusterLoader + ClusterVersionIndex,
    C: PrimitiveCatalog + PrimitiveVersionIndex,
{
    validate_cluster_definition(cluster_def)?;

    let mut ctx = ExpandContext::new();
    let build = expand_with_context(cluster_def, loader, catalog, &mut ctx, &[], &HashMap::new())?;

    let mut graph = build.graph;
    graph.boundary_inputs = cluster_def.input_ports.clone();
    graph.boundary_outputs = map_boundary_outputs(
        &cluster_def.output_ports,
        &build.node_mapping,
        &build.cluster_output_map,
    )?;

    // E.3 invariant: ExternalInput must not appear as edge target (sink) after expansion
    for edge in &graph.edges {
        if let ExpandedEndpoint::ExternalInput { name } = &edge.to {
            return Err(ExpandError::InvariantViolation(format!(
                "E.3: ExternalInput '{}' cannot be edge sink after expansion",
                name
            )));
        }
    }

    if let Some(declared) = &cluster_def.declared_signature {
        let inferred =
            infer_signature(&graph, catalog).map_err(ExpandError::SignatureInferenceFailed)?;
        validate_declared_signature(declared, &inferred)
            .map_err(ExpandError::DeclaredSignatureInvalid)?;
    }

    Ok(graph)
}

fn parse_available_versions(
    target_kind: VersionTargetKind,
    id: &str,
    available_versions: Vec<Version>,
) -> Result<Vec<(SemverVersion, Version)>, ExpandError> {
    let mut parsed = Vec::with_capacity(available_versions.len());
    for version in available_versions {
        let semver =
            SemverVersion::parse(&version).map_err(|_| ExpandError::InvalidAvailableVersion {
                target_kind,
                id: id.to_string(),
                version: version.clone(),
            })?;
        parsed.push((semver, version));
    }
    parsed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(parsed)
}

fn normalize_available_versions(parsed: &[(SemverVersion, Version)]) -> Vec<Version> {
    parsed.iter().map(|(_, raw)| raw.clone()).collect()
}

fn resolve_version_selector(
    target_kind: VersionTargetKind,
    id: &str,
    selector: &Version,
    available_versions: Vec<Version>,
) -> Result<Version, ExpandError> {
    if let Ok(exact) = SemverVersion::parse(selector) {
        if available_versions.is_empty() {
            // Preserve legacy topology-only expansion when no catalog/index entries exist.
            return Ok(exact.to_string());
        }

        let parsed = parse_available_versions(target_kind, id, available_versions)?;
        if let Some((matched, _)) = parsed.iter().find(|(candidate, _)| *candidate == exact) {
            return Ok(matched.to_string());
        }

        return Err(ExpandError::UnsatisfiedVersionConstraint {
            target_kind,
            id: id.to_string(),
            selector: selector.clone(),
            available_versions: normalize_available_versions(&parsed),
        });
    }

    let req = VersionReq::parse(selector).map_err(|_| ExpandError::InvalidVersionSelector {
        target_kind,
        id: id.to_string(),
        selector: selector.clone(),
    })?;

    let parsed = parse_available_versions(target_kind, id, available_versions)?;
    if let Some((matched, _)) = parsed
        .iter()
        .rev()
        .find(|(candidate, _)| req.matches(candidate))
    {
        return Ok(matched.to_string());
    }

    Err(ExpandError::UnsatisfiedVersionConstraint {
        target_kind,
        id: id.to_string(),
        selector: selector.clone(),
        available_versions: normalize_available_versions(&parsed),
    })
}

fn resolve_primitive_version<C: PrimitiveVersionIndex>(
    catalog: &C,
    impl_id: &str,
    selector: &Version,
) -> Result<Version, ExpandError> {
    resolve_version_selector(
        VersionTargetKind::Primitive,
        impl_id,
        selector,
        catalog.available_versions(impl_id),
    )
}

fn resolve_cluster_version<L: ClusterVersionIndex>(
    loader: &L,
    cluster_id: &str,
    selector: &Version,
) -> Result<Version, ExpandError> {
    resolve_version_selector(
        VersionTargetKind::Cluster,
        cluster_id,
        selector,
        loader.available_versions(cluster_id),
    )
}

fn validate_cluster_definition(cluster_def: &ClusterDefinition) -> Result<(), ExpandError> {
    let mut input_names = HashSet::new();
    for input in &cluster_def.input_ports {
        if !input_names.insert(input.name.clone()) {
            return Err(ExpandError::DuplicateInputPort {
                name: input.name.clone(),
            });
        }
    }

    let mut output_names = HashSet::new();
    for output in &cluster_def.output_ports {
        if !output_names.insert(output.name.clone()) {
            return Err(ExpandError::DuplicateOutputPort {
                name: output.name.clone(),
            });
        }
    }

    let mut parameter_names = HashSet::new();
    for param in &cluster_def.parameters {
        if !parameter_names.insert(param.name.clone()) {
            return Err(ExpandError::DuplicateParameter {
                name: param.name.clone(),
            });
        }

        if let Some(default) = &param.default {
            match default {
                ParameterDefault::Literal(v) => {
                    let got = parameter_value_type(v);
                    if got != param.ty {
                        return Err(ExpandError::ParameterDefaultTypeMismatch {
                            name: param.name.clone(),
                            expected: param.ty.clone(),
                            got,
                        });
                    }
                }
                ParameterDefault::DeriveKey { slot_name } => {
                    if param.ty != ParameterType::String {
                        return Err(ExpandError::ParameterDefaultTypeMismatch {
                            name: param.name.clone(),
                            expected: param.ty.clone(),
                            got: ParameterType::String,
                        });
                    }
                    if slot_name.is_empty() {
                        return Err(ExpandError::InvalidDeriveKeySlot {
                            parameter: param.name.clone(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn parameter_value_type(value: &ParameterValue) -> ParameterType {
    match value {
        ParameterValue::Int(_) => ParameterType::Int,
        ParameterValue::Number(_) => ParameterType::Number,
        ParameterValue::Bool(_) => ParameterType::Bool,
        ParameterValue::String(_) => ParameterType::String,
        ParameterValue::Enum(_) => ParameterType::Enum,
    }
}

/// Infers the cluster's signature from its expanded graph.
///
/// F.6 invariant: Inference depends only on:
/// - Graph structure (nodes, edges, boundary ports)
/// - Catalog (primitive metadata for node kind lookup)
///
/// Inference must NOT depend on runtime state, execution context,
/// or any mutable external state. This guarantees deterministic,
/// reproducible signatures for the same graph definition.
pub fn infer_signature<C: PrimitiveCatalog>(
    graph: &ExpandedGraph,
    catalog: &C,
) -> Result<Signature, SignatureInferenceError> {
    let mut node_meta: HashMap<String, PrimitiveMetadata> = HashMap::new();
    let mut has_side_effects = false;

    for (node_id, node) in &graph.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| SignatureInferenceError::MissingPrimitive {
                id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;
        if meta.kind == PrimitiveKind::Action {
            has_side_effects = true;
        }
        node_meta.insert(node_id.clone(), meta);
    }

    let mut inputs: Vec<PortSpec> = Vec::new();
    for input in &graph.boundary_inputs {
        let port = PortSpec {
            name: input.name.clone(),
            ty: input.maps_to.ty.clone(),
            cardinality: Cardinality::Single,
            wireable: false, // F.1: Input ports are never wireable
        };
        // F.1 invariant: Input ports must never be wireable (CLUSTER_SPEC.md §3.2)
        debug_assert!(
            !port.wireable,
            "Invariant F.1 violated: input port '{}' must not be wireable",
            port.name
        );
        inputs.push(port);
    }

    let mut outputs: Vec<PortSpec> = Vec::new();
    let mut has_wireable_outputs = false;
    let mut wireable_out_types: Vec<ValueType> = Vec::new();

    for output in &graph.boundary_outputs {
        let meta = node_meta
            .get(&output.maps_to.node_id)
            .ok_or_else(|| SignatureInferenceError::MissingNode(output.maps_to.node_id.clone()))?;

        let out_meta = meta.outputs.get(&output.maps_to.port_name).ok_or_else(|| {
            SignatureInferenceError::MissingOutput {
                impl_id: graph
                    .nodes
                    .get(&output.maps_to.node_id)
                    .map(|n| n.implementation.impl_id.clone())
                    .unwrap_or_default(),
                version: graph
                    .nodes
                    .get(&output.maps_to.node_id)
                    .map(|n| n.implementation.version.clone())
                    .unwrap_or_default(),
                output: output.maps_to.port_name.clone(),
            }
        })?;

        let wireable = meta.kind != PrimitiveKind::Action;
        if wireable {
            has_wireable_outputs = true;
            wireable_out_types.push(out_meta.value_type.clone());
        }

        outputs.push(PortSpec {
            name: output.name.clone(),
            ty: out_meta.value_type.clone(),
            cardinality: out_meta.cardinality.clone(),
            wireable,
        });
    }

    let has_wireable_event_out = wireable_out_types
        .iter()
        .any(|t| matches!(t, ValueType::Event));

    let kind = if !has_wireable_outputs {
        BoundaryKind::ActionLike
    } else if graph.boundary_inputs.is_empty()
        && wireable_out_types.iter().all(|t| {
            matches!(
                t,
                ValueType::Number | ValueType::Series | ValueType::Bool | ValueType::String
            )
        })
    {
        BoundaryKind::SourceLike
    } else if has_wireable_event_out {
        BoundaryKind::TriggerLike
    } else {
        BoundaryKind::ComputeLike
    };

    let is_origin = graph.boundary_inputs.is_empty() && roots_are_sources(graph, &node_meta);

    Ok(Signature {
        kind,
        inputs,
        outputs,
        has_side_effects,
        is_origin,
    })
}

/// D.11: Validate that declared signature wireability does not exceed inferred wireability.
/// Declared wireability can restrict (true → false) but cannot grant (false → true).
pub fn validate_declared_signature(
    declared: &Signature,
    inferred: &Signature,
) -> Result<(), ClusterValidationError> {
    // Check output ports: declared.wireable cannot exceed inferred.wireable
    for declared_port in &declared.outputs {
        if let Some(inferred_port) = inferred
            .outputs
            .iter()
            .find(|p| p.name == declared_port.name)
        {
            // D.11: If declared.wireable == true but inferred.wireable == false, reject
            if declared_port.wireable && !inferred_port.wireable {
                return Err(ClusterValidationError::WireabilityExceedsInferred {
                    port_name: declared_port.name.clone(),
                });
            }
        }
    }

    // Check input ports: declared.wireable cannot exceed inferred.wireable
    // Note: Per F.1, inferred inputs always have wireable: false, so any declared wireable: true is invalid
    for declared_port in &declared.inputs {
        if let Some(inferred_port) = inferred
            .inputs
            .iter()
            .find(|p| p.name == declared_port.name)
        {
            if declared_port.wireable && !inferred_port.wireable {
                return Err(ClusterValidationError::WireabilityExceedsInferred {
                    port_name: declared_port.name.clone(),
                });
            }
        }
    }

    Ok(())
}

fn roots_are_sources(graph: &ExpandedGraph, meta: &HashMap<String, PrimitiveMetadata>) -> bool {
    let mut incoming: HashSet<&String> = HashSet::new();
    for edge in &graph.edges {
        if let (
            ExpandedEndpoint::NodePort { node_id: _from, .. },
            ExpandedEndpoint::NodePort { node_id: to, .. },
        ) = (&edge.from, &edge.to)
        {
            incoming.insert(to);
        }
    }

    for node_id in graph.nodes.keys() {
        if !incoming.contains(node_id) {
            if let Some(m) = meta.get(node_id) {
                if m.kind != PrimitiveKind::Source {
                    return false;
                }
            } else {
                return false;
            }
        }
    }

    true
}

#[derive(Debug)]
struct ExpandContext {
    next_id: usize,
}

impl ExpandContext {
    fn new() -> Self {
        Self { next_id: 0 }
    }

    fn next_runtime_id(&mut self) -> String {
        let id = format!("n{}", self.next_id);
        self.next_id += 1;
        id
    }
}

/// I.3/I.4/I.5/I.7: Validate parameter bindings for a nested cluster instantiation.
fn validate_parameter_bindings(
    nested_def: &ClusterDefinition,
    bindings: &HashMap<String, ParameterBinding>,
    parent_parameters: &[ParameterSpec],
) -> Result<(), ExpandError> {
    // I.7: Reject bindings that reference undeclared parameters
    let spec_names: std::collections::HashSet<&str> = nested_def
        .parameters
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    for key in bindings.keys() {
        if !spec_names.contains(key.as_str()) {
            return Err(ExpandError::UndeclaredParameter {
                node_id: nested_def.id.clone(),
                parameter: key.clone(),
            });
        }
    }

    for param_spec in &nested_def.parameters {
        match bindings.get(&param_spec.name) {
            None => {
                // I.3: Required parameter with no default must have a binding
                if param_spec.required && param_spec.default.is_none() {
                    return Err(ExpandError::MissingRequiredParameter {
                        cluster_id: nested_def.id.clone(),
                        parameter: param_spec.name.clone(),
                    });
                }
            }
            Some(ParameterBinding::Literal { value }) => {
                // I.4: Literal binding must have correct type
                let got = parameter_value_type(value);
                if got != param_spec.ty {
                    return Err(ExpandError::ParameterBindingTypeMismatch {
                        cluster_id: nested_def.id.clone(),
                        parameter: param_spec.name.clone(),
                        expected: param_spec.ty.clone(),
                        got,
                    });
                }
            }
            Some(ParameterBinding::Exposed { parent_param }) => {
                // I.5: Exposed binding must reference existing parent parameter
                // I.4: Exposed binding must have compatible type
                let parent_spec = parent_parameters.iter().find(|p| &p.name == parent_param);
                match parent_spec {
                    None => {
                        return Err(ExpandError::ExposedParameterNotFound {
                            cluster_id: nested_def.id.clone(),
                            parameter: param_spec.name.clone(),
                            referenced: parent_param.clone(),
                        });
                    }
                    Some(spec) if spec.ty != param_spec.ty => {
                        return Err(ExpandError::ExposedParameterTypeMismatch {
                            cluster_id: nested_def.id.clone(),
                            parameter: param_spec.name.clone(),
                            expected: param_spec.ty.clone(),
                            got: spec.ty.clone(),
                        });
                    }
                    Some(_) => {} // Type matches, binding valid
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ExpandBuild {
    graph: ExpandedGraph,
    node_mapping: HashMap<NodeId, String>,
    placeholder_map: HashMap<String, String>,
    cluster_output_map: HashMap<NodeId, HashMap<String, ExpandedEndpoint>>,
}

fn expand_with_context<L, C>(
    cluster_def: &ClusterDefinition,
    loader: &L,
    catalog: &C,
    ctx: &mut ExpandContext,
    authoring_prefix: &[(String, NodeId)],
    resolved_params: &HashMap<String, ParameterValue>,
) -> Result<ExpandBuild, ExpandError>
where
    L: ClusterLoader + ClusterVersionIndex,
    C: PrimitiveCatalog + PrimitiveVersionIndex,
{
    if cluster_def.nodes.is_empty() {
        return Err(ExpandError::EmptyCluster);
    }

    let placeholder_map =
        build_placeholder_map(authoring_prefix, &cluster_def.id, &cluster_def.input_ports);

    let mut graph = ExpandedGraph {
        nodes: HashMap::new(),
        edges: Vec::new(),
        boundary_inputs: Vec::new(),
        boundary_outputs: Vec::new(),
    };
    let mut node_mapping: HashMap<NodeId, String> = HashMap::new();
    let mut cluster_output_map: HashMap<NodeId, HashMap<String, ExpandedEndpoint>> = HashMap::new();
    let mut cluster_input_map: HashMap<NodeId, HashMap<String, String>> = HashMap::new();

    // C.1: Iterate nodes in sorted key order for deterministic runtime_id assignment
    let mut sorted_node_ids: Vec<_> = cluster_def.nodes.keys().collect();
    sorted_node_ids.sort();
    for node_id in sorted_node_ids {
        let node = cluster_def.nodes.get(node_id).unwrap();
        match &node.kind {
            NodeKind::Impl { impl_id, version } => {
                let runtime_id = ctx.next_runtime_id();
                let mut authoring_path = authoring_prefix.to_vec();
                authoring_path.push((cluster_def.id.clone(), node.id.clone()));

                let resolved_version = resolve_primitive_version(catalog, impl_id, version)?;
                // A.1: Look up primitive specs to get parameter defaults using resolved semver.
                let primitive_meta = catalog.get(impl_id, &resolved_version);

                // A.1: Resolve parameters:
                // - If catalog has metadata, use specs to apply defaults
                // - Otherwise, fall back to direct binding resolution (legacy)
                let resolved_bindings = if let Some(ref meta) = primitive_meta {
                    resolve_impl_parameters(
                        &node.id,
                        &meta.parameters,
                        &node.parameter_bindings,
                        resolved_params,
                    )?
                } else {
                    // Legacy path: resolve bindings without spec validation
                    resolve_bindings_with_context(
                        &node.id,
                        &node.parameter_bindings,
                        resolved_params,
                    )?
                };

                graph.nodes.insert(
                    runtime_id.clone(),
                    ExpandedNode {
                        runtime_id: runtime_id.clone(),
                        authoring_path,
                        implementation: ImplementationInstance {
                            impl_id: impl_id.clone(),
                            requested_version: version.clone(),
                            version: resolved_version,
                        },
                        parameters: resolved_bindings,
                    },
                );

                node_mapping.insert(node.id.clone(), runtime_id);
            }
            NodeKind::Cluster {
                cluster_id,
                version,
            } => {
                let resolved_cluster_version =
                    resolve_cluster_version(loader, cluster_id, version)?;
                let nested_def = loader
                    .load(cluster_id, &resolved_cluster_version)
                    .ok_or_else(|| ExpandError::MissingCluster {
                        id: cluster_id.clone(),
                        version: resolved_cluster_version.clone(),
                    })?;

                // I.3/I.4/I.5: Validate parameter bindings before expansion
                validate_parameter_bindings(
                    &nested_def,
                    &node.parameter_bindings,
                    &cluster_def.parameters,
                )?;

                let bound_nested = apply_literal_bindings(&nested_def, &node.parameter_bindings);

                // Compute nested authoring path before build_resolved_params
                // so DeriveKey defaults can access the instantiation path.
                let mut nested_prefix = authoring_prefix.to_vec();
                nested_prefix.push((cluster_def.id.clone(), node.id.clone()));

                // A.1: Build resolved parameter values for the nested cluster:
                // - Literal bindings use their value directly
                // - Exposed bindings look up the value from our resolved_params
                // - Missing bindings use defaults from cluster parameter specs
                // - DeriveKey defaults derive deterministic keys from the authoring path
                let nested_resolved_params = build_resolved_params(
                    &nested_def.id,
                    &nested_def.parameters,
                    &node.parameter_bindings,
                    resolved_params,
                    &nested_prefix,
                )?;

                let nested_build = expand_with_context(
                    &bound_nested,
                    loader,
                    catalog,
                    ctx,
                    &nested_prefix,
                    &nested_resolved_params,
                )?;

                merge_graph(&mut graph, nested_build.graph);

                let mut input_map: HashMap<String, String> = HashMap::new();
                for input_port in &bound_nested.input_ports {
                    if let Some(mapped) = nested_build.placeholder_map.get(&input_port.maps_to.name)
                    {
                        input_map.insert(input_port.name.clone(), mapped.clone());
                    }
                }
                cluster_input_map.insert(node.id.clone(), input_map);

                // D.4: Map all output ports, failing if any node_id is unmapped
                let mut output_map: HashMap<String, ExpandedEndpoint> = HashMap::new();
                for output_port in &bound_nested.output_ports {
                    let mapped_output = resolve_mapped_output(
                        &output_port.maps_to,
                        &nested_build.node_mapping,
                        &nested_build.cluster_output_map,
                    )
                    .ok_or_else(|| ExpandError::UnmappedNestedOutput {
                        cluster_id: node.id.clone(),
                        port_name: output_port.name.clone(),
                    })?;
                    let ExpandedEndpoint::NodePort { node_id, port_name } = mapped_output else {
                        return Err(ExpandError::UnmappedNestedOutput {
                            cluster_id: node.id.clone(),
                            port_name: output_port.name.clone(),
                        });
                    };
                    output_map.insert(
                        output_port.name.clone(),
                        ExpandedEndpoint::NodePort { node_id, port_name },
                    );
                }
                cluster_output_map.insert(node.id.clone(), output_map);

                for (k, v) in nested_build.node_mapping {
                    node_mapping.insert(k, v);
                }
            }
        }
    }

    for edge in &cluster_def.edges {
        let from = resolve_output_endpoint(
            &edge.from,
            &node_mapping,
            &cluster_output_map,
            authoring_prefix,
            &cluster_def.id,
        );
        let to = resolve_input_endpoint(
            &edge.to,
            &node_mapping,
            &cluster_input_map,
            &placeholder_map,
            authoring_prefix,
            &cluster_def.id,
        );

        if let ExpandedEndpoint::ExternalInput { name } = &to {
            let replaced = redirect_placeholder_edges(&mut graph.edges, name, &from);
            if !replaced {
                graph.edges.push(ExpandedEdge {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        } else {
            graph.edges.push(ExpandedEdge { from, to });
        }
    }

    Ok(ExpandBuild {
        graph,
        node_mapping,
        placeholder_map,
        cluster_output_map,
    })
}

fn build_placeholder_map(
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
    input_ports: &[InputPortSpec],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for input in input_ports {
        let key = external_key(authoring_prefix, cluster_id, &input.maps_to.name);
        map.insert(input.maps_to.name.clone(), key);
    }
    map
}

fn external_key(authoring_prefix: &[(String, NodeId)], cluster_id: &str, name: &str) -> String {
    let mut parts: Vec<String> = authoring_prefix
        .iter()
        .map(|(c, n)| format!("{}:{}", c, n))
        .collect();
    parts.push(cluster_id.to_string());
    parts.push(name.to_string());
    parts.join("/")
}

fn merge_graph(target: &mut ExpandedGraph, nested: ExpandedGraph) {
    for (id, node) in nested.nodes {
        target.nodes.insert(id, node);
    }
    target.edges.extend(nested.edges);
}

fn resolve_output_endpoint(
    output: &OutputRef,
    node_mapping: &HashMap<NodeId, String>,
    cluster_output_map: &HashMap<NodeId, HashMap<String, ExpandedEndpoint>>,
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
) -> ExpandedEndpoint {
    if let Some(node_id) = node_mapping.get(&output.node_id) {
        return ExpandedEndpoint::NodePort {
            node_id: node_id.clone(),
            port_name: output.port_name.clone(),
        };
    }

    if let Some(map) = cluster_output_map.get(&output.node_id) {
        if let Some(ep) = map.get(&output.port_name) {
            return ep.clone();
        }
    }

    ExpandedEndpoint::ExternalInput {
        name: external_key(authoring_prefix, cluster_id, &output.node_id),
    }
}

fn resolve_mapped_output(
    output: &OutputRef,
    node_mapping: &HashMap<NodeId, String>,
    cluster_output_map: &HashMap<NodeId, HashMap<String, ExpandedEndpoint>>,
) -> Option<ExpandedEndpoint> {
    if let Some(node_id) = node_mapping.get(&output.node_id) {
        return Some(ExpandedEndpoint::NodePort {
            node_id: node_id.clone(),
            port_name: output.port_name.clone(),
        });
    }

    cluster_output_map
        .get(&output.node_id)
        .and_then(|map| map.get(&output.port_name).cloned())
}

fn resolve_input_endpoint(
    input: &InputRef,
    node_mapping: &HashMap<NodeId, String>,
    cluster_input_map: &HashMap<NodeId, HashMap<String, String>>,
    placeholder_map: &HashMap<String, String>,
    authoring_prefix: &[(String, NodeId)],
    cluster_id: &str,
) -> ExpandedEndpoint {
    if let Some(node_id) = node_mapping.get(&input.node_id) {
        return ExpandedEndpoint::NodePort {
            node_id: node_id.clone(),
            port_name: input.port_name.clone(),
        };
    }

    if let Some(map) = cluster_input_map.get(&input.node_id) {
        if let Some(name) = map.get(&input.port_name) {
            return ExpandedEndpoint::ExternalInput { name: name.clone() };
        }
    }

    if let Some(name) = placeholder_map.get(&input.node_id) {
        return ExpandedEndpoint::ExternalInput { name: name.clone() };
    }

    ExpandedEndpoint::ExternalInput {
        name: external_key(authoring_prefix, cluster_id, &input.node_id),
    }
}

fn redirect_placeholder_edges(
    edges: &mut [ExpandedEdge],
    placeholder: &str,
    source: &ExpandedEndpoint,
) -> bool {
    let mut replaced = false;
    for edge in edges.iter_mut() {
        if let ExpandedEndpoint::ExternalInput { name } = &edge.from {
            if name == placeholder {
                edge.from = source.clone();
                replaced = true;
            }
        }
    }
    replaced
}

fn apply_literal_bindings(
    cluster_def: &ClusterDefinition,
    bindings: &HashMap<String, ParameterBinding>,
) -> ClusterDefinition {
    // Clone is local to this call; the original ClusterDefinition is never mutated.
    let mut updated = cluster_def.clone();
    for node in updated.nodes.values_mut() {
        for binding in node.parameter_bindings.values_mut() {
            if let ParameterBinding::Exposed { parent_param } = binding {
                if let Some(ParameterBinding::Literal { value }) = bindings.get(parent_param) {
                    *binding = ParameterBinding::Literal {
                        value: value.clone(),
                    };
                }
            }
        }
    }
    updated
}

/// Resolves parameter bindings for a primitive node using the parent's resolved parameters.
/// - Literal bindings use their value directly
/// - Exposed bindings look up the value from resolved_params
/// - Unresolved exposed bindings produce an error
fn resolve_bindings_with_context(
    node_id: &str,
    bindings: &HashMap<String, ParameterBinding>,
    resolved_params: &HashMap<String, ParameterValue>,
) -> Result<HashMap<String, ParameterValue>, ExpandError> {
    let mut result = HashMap::new();
    for (name, binding) in bindings {
        match binding {
            ParameterBinding::Literal { value } => {
                result.insert(name.clone(), value.clone());
            }
            ParameterBinding::Exposed { parent_param } => {
                // Look up the value from parent's resolved parameters
                if let Some(value) = resolved_params.get(parent_param) {
                    result.insert(name.clone(), value.clone());
                } else {
                    return Err(ExpandError::UnresolvedExposedBinding {
                        node_id: node_id.to_string(),
                        parameter: name.clone(),
                        referenced: parent_param.clone(),
                    });
                }
            }
        }
    }
    Ok(result)
}

/// A.1: Resolves parameters for a primitive node using specs, bindings, and defaults.
/// - Explicit bindings (Literal or Exposed) take precedence
/// - If no binding, apply default from primitive spec
/// - If no binding and no default and required, error
fn resolve_impl_parameters(
    node_id: &str,
    specs: &[ParameterMetadata],
    bindings: &HashMap<String, ParameterBinding>,
    parent_resolved: &HashMap<String, ParameterValue>,
) -> Result<HashMap<String, ParameterValue>, ExpandError> {
    // I.7: Reject bindings that reference undeclared parameters
    let spec_names: std::collections::HashSet<&str> =
        specs.iter().map(|s| s.name.as_str()).collect();
    for key in bindings.keys() {
        if !spec_names.contains(key.as_str()) {
            return Err(ExpandError::UndeclaredParameter {
                node_id: node_id.to_string(),
                parameter: key.clone(),
            });
        }
    }

    let mut result = HashMap::new();

    for spec in specs {
        match bindings.get(&spec.name) {
            Some(ParameterBinding::Literal { value }) => {
                result.insert(spec.name.clone(), value.clone());
            }
            Some(ParameterBinding::Exposed { parent_param }) => {
                if let Some(value) = parent_resolved.get(parent_param) {
                    result.insert(spec.name.clone(), value.clone());
                } else {
                    return Err(ExpandError::UnresolvedExposedBinding {
                        node_id: node_id.to_string(),
                        parameter: spec.name.clone(),
                        referenced: parent_param.clone(),
                    });
                }
            }
            None => {
                // A.1: Apply default if available
                if let Some(default) = &spec.default {
                    result.insert(spec.name.clone(), default.clone());
                } else if spec.required {
                    return Err(ExpandError::MissingRequiredParameter {
                        cluster_id: node_id.to_string(),
                        parameter: spec.name.clone(),
                    });
                }
                // else: optional with no default, omit
            }
        }
    }

    Ok(result)
}

/// A.1: Builds resolved parameter values for a nested cluster instantiation.
/// - Explicit bindings (Literal or Exposed) take precedence
/// - If no binding, apply default from cluster parameter spec
/// - If no binding and no default and required, error
///
/// Note: This function is called only for nested cluster instantiation.
/// Root-cluster parameter defaults are not resolved through this path;
/// `derive_key` defaults are for nested cluster instantiation only.
fn build_resolved_params(
    cluster_id: &str,
    specs: &[ParameterSpec],
    bindings: &HashMap<String, ParameterBinding>,
    resolved_params: &HashMap<String, ParameterValue>,
    authoring_path: &[(String, NodeId)],
) -> Result<HashMap<String, ParameterValue>, ExpandError> {
    // I.7: Reject bindings that reference undeclared parameters
    let spec_names: std::collections::HashSet<&str> =
        specs.iter().map(|s| s.name.as_str()).collect();
    for key in bindings.keys() {
        if !spec_names.contains(key.as_str()) {
            return Err(ExpandError::UndeclaredParameter {
                node_id: cluster_id.to_string(),
                parameter: key.clone(),
            });
        }
    }

    let mut result = HashMap::new();

    for spec in specs {
        match bindings.get(&spec.name) {
            Some(ParameterBinding::Literal { value }) => {
                result.insert(spec.name.clone(), value.clone());
            }
            Some(ParameterBinding::Exposed { parent_param }) => {
                if let Some(value) = resolved_params.get(parent_param) {
                    result.insert(spec.name.clone(), value.clone());
                } else {
                    return Err(ExpandError::UnresolvedExposedBinding {
                        node_id: cluster_id.to_string(),
                        parameter: spec.name.clone(),
                        referenced: parent_param.clone(),
                    });
                }
            }
            None => {
                // A.1: Apply default if available
                if let Some(default) = &spec.default {
                    match default {
                        ParameterDefault::Literal(v) => {
                            result.insert(spec.name.clone(), v.clone());
                        }
                        ParameterDefault::DeriveKey { slot_name } => {
                            result.insert(
                                spec.name.clone(),
                                ParameterValue::String(derive_key(authoring_path, slot_name)),
                            );
                        }
                    }
                } else if spec.required {
                    return Err(ExpandError::MissingRequiredParameter {
                        cluster_id: cluster_id.to_string(),
                        parameter: spec.name.clone(),
                    });
                }
                // else: optional with no default, omit
            }
        }
    }

    Ok(result)
}

/// D.4: Map boundary outputs, failing if any node_id is unmapped
fn map_boundary_outputs(
    outputs: &[OutputPortSpec],
    mapping: &HashMap<NodeId, String>,
    cluster_output_map: &HashMap<NodeId, HashMap<String, ExpandedEndpoint>>,
) -> Result<Vec<OutputPortSpec>, ExpandError> {
    let mut result = Vec::with_capacity(outputs.len());
    for o in outputs {
        let mapped_output = resolve_mapped_output(&o.maps_to, mapping, cluster_output_map)
            .ok_or_else(|| ExpandError::UnmappedBoundaryOutput {
                port_name: o.name.clone(),
                node_id: o.maps_to.node_id.clone(),
            })?;
        let ExpandedEndpoint::NodePort { node_id, port_name } = mapped_output else {
            return Err(ExpandError::UnmappedBoundaryOutput {
                port_name: o.name.clone(),
                node_id: o.maps_to.node_id.clone(),
            });
        };
        result.push(OutputPortSpec {
            name: o.name.clone(),
            maps_to: OutputRef { node_id, port_name },
        });
    }
    Ok(result)
}

/// Derive a deterministic, injective key from an authoring path and slot name.
///
/// Uses length-prefixed segments to avoid delimiter collisions (identifiers may
/// contain `#`, `/`, etc.). The output is namespaced with `__ergo/` to avoid
/// collisions with user-chosen key names.
///
/// Format: `__ergo/<len>:<segment>/<len>:<segment>/.../<len>:<slot_name>`
/// where `<len>` is the UTF-8 byte length of the following segment,
/// and segments alternate cluster_id and node_id from the authoring path.
pub fn derive_key(authoring_path: &[(String, NodeId)], slot_name: &str) -> String {
    let mut parts = Vec::new();
    for (cluster_id, node_id) in authoring_path {
        parts.push(format!("{}:{}", cluster_id.len(), cluster_id));
        parts.push(format!("{}:{}", node_id.len(), node_id));
    }
    parts.push(format!("{}:{}", slot_name.len(), slot_name));
    format!("__ergo/{}", parts.join("/"))
}

#[cfg(test)]
mod tests;
