//! usecases node_analysis
//!
//! Purpose:
//! - Resolve source/action nodes in an expanded graph against the core catalog
//!   and registries for host-side dependency scanning and adapter-composition
//!   validation.
//!
//! Owns:
//! - One shared source/action node walk over `ExpandedGraph` that surfaces
//!   missing catalog or registry entries with enough context for the host
//!   facade to map into public errors.
//!
//! Does not own:
//! - Public host error enums or adapter/dependency semantics.
//! - Compute/trigger validation; the current host-side callers only inspect
//!   source/action nodes.
//!
//! Connects to:
//! - `usecases.rs`, which maps these resolved nodes into
//!   `scan_adapter_dependencies(...)` and `validate_adapter_composition(...)`.
//!
//! Safety notes:
//! - This helper is intentionally host-private so the public facade can keep
//!   distinct error surfaces while sharing one registry-resolution authority.

use crate::usecases::shared::*;

pub(super) enum ResolvedHostNode<'a> {
    Source {
        runtime_id: &'a str,
        node: &'a ergo_runtime::cluster::ExpandedNode,
        source: &'a dyn ergo_runtime::source::SourcePrimitive,
    },
    Action {
        runtime_id: &'a str,
        node: &'a ergo_runtime::cluster::ExpandedNode,
        action: &'a dyn ergo_runtime::action::ActionPrimitive,
    },
}

#[derive(Debug)]
pub(super) enum ResolveHostNodeError {
    MissingCatalogMetadata {
        runtime_id: String,
        primitive_id: String,
        version: Version,
    },
    MissingSourcePrimitive {
        runtime_id: String,
        primitive_id: String,
    },
    MissingActionPrimitive {
        runtime_id: String,
        primitive_id: String,
    },
}

pub(super) fn resolve_source_and_action_nodes<'a>(
    expanded: &'a ExpandedGraph,
    catalog: &'a CorePrimitiveCatalog,
    registries: &'a CoreRegistries,
) -> Result<Vec<ResolvedHostNode<'a>>, ResolveHostNodeError> {
    let mut resolved = Vec::new();

    for (runtime_id, node) in &expanded.nodes {
        let meta = catalog
            .get(&node.implementation.impl_id, &node.implementation.version)
            .ok_or_else(|| ResolveHostNodeError::MissingCatalogMetadata {
                runtime_id: runtime_id.clone(),
                primitive_id: node.implementation.impl_id.clone(),
                version: node.implementation.version.clone(),
            })?;

        match meta.kind {
            PrimitiveKind::Source => {
                let source = registries
                    .sources
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| ResolveHostNodeError::MissingSourcePrimitive {
                        runtime_id: runtime_id.clone(),
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                resolved.push(ResolvedHostNode::Source {
                    runtime_id,
                    node,
                    source,
                });
            }
            PrimitiveKind::Action => {
                let action = registries
                    .actions
                    .get(&node.implementation.impl_id)
                    .ok_or_else(|| ResolveHostNodeError::MissingActionPrimitive {
                        runtime_id: runtime_id.clone(),
                        primitive_id: node.implementation.impl_id.clone(),
                    })?;
                resolved.push(ResolvedHostNode::Action {
                    runtime_id,
                    node,
                    action,
                });
            }
            _ => {}
        }
    }

    Ok(resolved)
}
