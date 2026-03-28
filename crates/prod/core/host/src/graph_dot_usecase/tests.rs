//! graph_dot_usecase::tests
//!
//! Purpose:
//! - Smoke-test that the DOT rendering pipeline produces expected output from
//!   an in-memory graph, keeping the production module free of test fixtures.

use super::*;

use ergo_loader::{load_graph_assets_from_memory, InMemorySourceInput};

#[test]
fn graph_to_dot_from_assets_renders_loaded_in_memory_graph() -> Result<(), String> {
    let assets = load_graph_assets_from_memory(
        "mem/root.yaml",
        &[InMemorySourceInput {
            source_id: "mem/root.yaml".to_string(),
            source_label: "mem/root.yaml".to_string(),
            content: r#"
kind: cluster
id: memory_visual
version: "0.1.0"
nodes:
  src:
    impl: number_source@0.1.0
    params:
      value: 3
  cmp:
    impl: gt@0.1.0
edges:
  - src.value -> cmp.a
outputs:
  out: cmp.result
"#
            .to_string(),
        }],
        &[],
    )
    .map_err(|err| err.to_string())?;

    let dot = graph_to_dot_from_assets(GraphToDotFromAssetsRequest {
        assets,
        show_ports: true,
        show_impl: false,
        show_runtime_id: false,
    })?;

    assert!(dot.contains("digraph \"memory_visual\""));
    assert!(dot.contains("src"));
    assert!(dot.contains("cmp"));
    assert!(dot.contains("value -> a"));
    Ok(())
}
