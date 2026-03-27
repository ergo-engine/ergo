use std::fs;
use std::path::{Path, PathBuf};

use ergo_runtime::common::Phase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleStatus {
    Enforced,
    Deferred,
    /// This rule is satisfied by enforcement of another rule ID.
    Alias {
        enforced_by: &'static str,
    },
}

impl RuleStatus {
    fn as_str(&self) -> String {
        match self {
            RuleStatus::Enforced => "enforced".to_string(),
            RuleStatus::Deferred => "deferred".to_string(),
            RuleStatus::Alias { enforced_by } => format!("alias -> {enforced_by}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleDefinition {
    pub id: &'static str,
    pub phase: Phase,
    pub summary: &'static str,
    pub predicate: &'static str,
    pub status: RuleStatus,
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Registration => "Registration",
        Phase::Composition => "Composition",
        Phase::Execution => "Execution",
    }
}

fn repo_root() -> PathBuf {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").exists() && candidate.join("docs").exists() {
            return candidate.to_path_buf();
        }
    }
    PathBuf::from(".")
}

fn registry_output_path() -> PathBuf {
    let root = repo_root();
    let canonical_dir = root.join("docs/invariants");
    let canonical = canonical_dir.join("rule-registry.md");
    if canonical.exists() || canonical_dir.exists() {
        return canonical;
    }

    let stable = root.join("docs/STABLE/RULE_REGISTRY.md");
    if stable.exists() {
        return stable;
    }

    let legacy = root.join("docs_legacy/STABLE/RULE_REGISTRY.md");
    if legacy.exists() {
        return legacy;
    }

    canonical
}

pub fn gen_docs_command(args: &[String]) -> Result<String, String> {
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            _ => return Err(usage()),
        }
    }

    let out_path = registry_output_path();
    let generated = generate_rule_registry_markdown();

    if check {
        let existing = fs::read_to_string(&out_path)
            .map_err(|err| format!("read {}: {err}", out_path.display()))?;
        if normalize_newlines(&existing) != normalize_newlines(&generated) {
            return Err(format!(
                "generated docs do not match committed file: {}\n(run: ergo gen-docs to update)",
                out_path.display()
            ));
        }
        return Ok("✓ Docs up-to-date\n".to_string());
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    fs::write(&out_path, &generated)
        .map_err(|err| format!("write {}: {err}", out_path.display()))?;

    Ok(format!("✓ Generated {}\n", out_path.display()))
}

fn usage() -> String {
    "usage: ergo gen-docs [--check]".to_string()
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn escape_table_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn current_rule_doc_anchor(rule_id: &str) -> &'static str {
    match rule_id {
        id if id.starts_with("ADP-") => "docs/primitives/adapter.md#43-enforcement-mapping",
        "SRC-10" | "SRC-11" => "docs/primitives/source.md#42-composition-rules",
        id if id.starts_with("SRC-") => "docs/primitives/source.md#43-enforcement-mapping",
        id if id.starts_with("CMP-") => "docs/primitives/compute.md#4-enforcement-mapping",
        id if id.starts_with("TRG-") => "docs/primitives/trigger.md#4-enforcement-mapping",
        id if id.starts_with("ACT-") => "docs/primitives/action.md#5-enforcement-mapping",
        "COMP-1" | "COMP-2" | "COMP-3" => {
            "docs/primitives/adapter.md#51-composition-enforcement-mapping"
        }
        "COMP-4" | "COMP-5" | "COMP-6" => "docs/primitives/compute.md#5-composition-rules",
        "COMP-7" | "COMP-8" => "docs/primitives/trigger.md#5-composition-rules",
        "COMP-9" | "COMP-10" | "COMP-11" | "COMP-12" | "COMP-13" | "COMP-14" | "COMP-15"
        | "COMP-17" | "COMP-18" | "COMP-19" => "docs/primitives/action.md#6-composition-rules",
        "COMP-16" => "docs/invariants/10-adapter-composition.md",
        id if id.starts_with("D.")
            || id.starts_with("I.")
            || id.starts_with("E.")
            || id.starts_with("V.") =>
        {
            "docs/authoring/cluster-spec.md#64-enforcement-mapping-phase-6"
        }
        _ => "docs/invariants/INDEX.md",
    }
}

fn generate_rule_registry_markdown() -> String {
    let mut out = String::new();

    out.push_str("---\n");
    out.push_str("Authority: STABLE\n");
    out.push_str("Version: v0\n");
    out.push_str("Generated: true\n");
    out.push_str("---\n\n");
    out.push_str("# Rule Registry (Generated)\n\n");
    out.push_str("This file is generated by `ergo gen-docs` from the code-side rule registry.\n");
    out.push_str("Do not edit by hand.\n\n");
    out.push_str("Scope note: this generated index covers the rule families enumerated below, not every live rule id in the repo; cross-phase and other out-of-band doctrine remain authoritative in code and `docs/invariants/*.md`.\n\n");

    render_group(
        &mut out,
        "Adapter (ADP-*)",
        ADAPTER_RULES,
        "docs/primitives/adapter.md",
    );
    render_group(
        &mut out,
        "Source (SRC-*)",
        SOURCE_RULES,
        "docs/primitives/source.md",
    );
    render_group(
        &mut out,
        "Compute (CMP-*)",
        COMPUTE_RULES,
        "docs/primitives/compute.md",
    );
    render_group(
        &mut out,
        "Trigger (TRG-*)",
        TRIGGER_RULES,
        "docs/primitives/trigger.md",
    );
    render_group(
        &mut out,
        "Action (ACT-*)",
        ACTION_RULES,
        "docs/primitives/action.md",
    );
    render_group(
        &mut out,
        "Composition (COMP-*)",
        COMPOSITION_RULES,
        "docs/primitives/*.md (distributed) + docs/invariants/10-adapter-composition.md",
    );
    render_group(
        &mut out,
        "Cluster (D./I./E./V.)",
        CLUSTER_RULES,
        "docs/authoring/cluster-spec.md",
    );

    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn render_group(out: &mut String, title: &str, rules: &[RuleDefinition], primary_doc: &str) {
    out.push_str(&format!("## {title}\n\n"));
    out.push_str(&format!("Primary spec: `{primary_doc}`\n\n"));
    out.push_str("| Rule ID | Phase | Status | Summary | Predicate | Docs |\n");
    out.push_str("|---------|-------|--------|---------|-----------|------|\n");

    for rule in rules {
        let summary = escape_table_cell(rule.summary);
        let predicate = format!("`{}`", escape_table_cell(rule.predicate));
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | `{}` |\n",
            rule.id,
            phase_name(rule.phase),
            rule.status.as_str(),
            summary,
            predicate,
            current_rule_doc_anchor(rule.id)
        ));
    }

    out.push('\n');
}

// ---- Rule sets (mirror of STABLE docs) ---------------------------------

const ADAPTER_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "ADP-1",
        phase: Phase::Registration,
        summary: "ID format valid",
        predicate: "regex(id, /^[a-z][a-z0-9_]*$/)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-2",
        phase: Phase::Registration,
        summary: "Version valid semver",
        predicate: "semver.valid(version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-3",
        phase: Phase::Registration,
        summary: "Runtime compatibility satisfied",
        predicate: "runtime.version >= runtime_compatibility",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-4",
        phase: Phase::Registration,
        summary: "Provides something",
        predicate: "context_keys.len > 0 OR event_kinds.len > 0",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-5",
        phase: Phase::Registration,
        summary: "Context key names unique",
        predicate: "unique(context_keys[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-6",
        phase: Phase::Registration,
        summary: "Context key types valid",
        predicate: "all(context_keys[].type in {Number, Bool, String, Series})",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-7",
        phase: Phase::Registration,
        summary: "Event kind names unique",
        predicate: "unique(event_kinds[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-8",
        phase: Phase::Registration,
        summary: "Event schemas valid JSON Schema",
        predicate: "json_schema.validate(payload_schema, draft: 2020-12)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-9",
        phase: Phase::Registration,
        summary: "Capture format version present",
        predicate: "capture.format_version != \"\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-10",
        phase: Phase::Registration,
        summary: "Capture fields referentially valid",
        predicate: "all(capture.fields[] in CaptureFieldSet(adapter))",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-11",
        phase: Phase::Registration,
        summary: "Writable flag must be present",
        predicate: "all(context_keys[].writable is present)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-12",
        phase: Phase::Registration,
        summary: "Effect names unique",
        predicate: "unique(accepts.effects[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-13",
        phase: Phase::Registration,
        summary: "Effect schemas valid",
        predicate: "all(accepts.effects[].payload_schema is valid Draft 2020-12)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-14",
        phase: Phase::Registration,
        summary: "Writable implies set_context accepted",
        predicate: "any(writable == true) => accepts contains \"set_context\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-15",
        phase: Phase::Registration,
        summary: "Writable keys must be capturable (planned)",
        predicate: "all(writable == true => \"context.\" + name in capture.fields)",
        status: RuleStatus::Deferred,
    },
    RuleDefinition {
        id: "ADP-16",
        phase: Phase::Registration,
        summary: "Write effect must be capturable (planned)",
        predicate: "any(writable == true) => \"effect.set_context\" in capture.fields",
        status: RuleStatus::Deferred,
    },
    RuleDefinition {
        id: "ADP-17",
        phase: Phase::Registration,
        summary: "Writable keys cannot be required",
        predicate: "all(writable == true => required == false)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-18",
        phase: Phase::Registration,
        summary: "Required event fields map to context keys with compatible types",
        predicate: "all(required(event.payload_schema) fields exist in context_keys with matching ValueType)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ADP-19",
        phase: Phase::Registration,
        summary: "Materialized event field types are supported",
        predicate: "event payload object fields map only to Number/Bool/String/Series",
        status: RuleStatus::Enforced,
    },
];

const SOURCE_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "SRC-1",
        phase: Phase::Registration,
        summary: "ID format valid",
        predicate: "regex(id, /^[a-z][a-z0-9_]*$/)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-2",
        phase: Phase::Registration,
        summary: "Version valid semver",
        predicate: "semver.valid(version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-3",
        phase: Phase::Registration,
        summary: "Kind is \"source\"",
        predicate: "kind == \"source\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-4",
        phase: Phase::Registration,
        summary: "No inputs declared",
        predicate: "inputs.len == 0",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-5",
        phase: Phase::Registration,
        summary: "At least one output",
        predicate: "outputs.len >= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-6",
        phase: Phase::Registration,
        summary: "Output names unique",
        predicate: "unique(outputs[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-7",
        phase: Phase::Registration,
        summary: "Output types valid",
        predicate: "all(outputs[].type ∈ ValueType)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-8",
        phase: Phase::Registration,
        summary: "State not allowed",
        predicate: "state.allowed == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-9",
        phase: Phase::Registration,
        summary: "Side effects not allowed",
        predicate: "side_effects == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-12",
        phase: Phase::Registration,
        summary: "Execution deterministic",
        predicate: "execution.deterministic == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-13",
        phase: Phase::Registration,
        summary: "Cadence is continuous",
        predicate: "execution.cadence == continuous",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-14",
        phase: Phase::Registration,
        summary: "ID unique in registry",
        predicate: "!registry.contains_key(id)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-15",
        phase: Phase::Registration,
        summary: "Parameter default type matches declared type",
        predicate:
            "parameters[].default == None || typeof(parameters[].default) == parameters[].type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-16",
        phase: Phase::Registration,
        summary: "$key context references bound to declared parameter",
        predicate: "∀ ctx where name starts with \"$\": referenced param exists",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-17",
        phase: Phase::Registration,
        summary: "$key context references must be String type",
        predicate: "∀ ctx where name starts with \"$\": referenced param.type == String",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "SRC-10",
        phase: Phase::Composition,
        summary: "Required context keys exist in adapter",
        predicate: "source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys",
        status: RuleStatus::Alias {
            enforced_by: "COMP-1",
        },
    },
    RuleDefinition {
        id: "SRC-11",
        phase: Phase::Composition,
        summary: "Provided context types match adapter",
        predicate:
            "∀ k in source.requires.context where adapter provides k: source.requires.context[k].ty == adapter.provides.context[k].ty",
        status: RuleStatus::Alias {
            enforced_by: "COMP-2",
        },
    },
];

const COMPUTE_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "CMP-1",
        phase: Phase::Registration,
        summary: "ID format valid",
        predicate: "regex(id, /^[a-z][a-z0-9_]*$/)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-2",
        phase: Phase::Registration,
        summary: "Version valid semver",
        predicate: "semver.valid(version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-3",
        phase: Phase::Registration,
        summary: "Kind is \"compute\"",
        predicate: "kind == \"compute\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-4",
        phase: Phase::Registration,
        summary: "At least one input",
        predicate: "inputs.len >= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-5",
        phase: Phase::Registration,
        summary: "Input names unique",
        predicate: "unique(inputs[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-6",
        phase: Phase::Registration,
        summary: "At least one output",
        predicate: "outputs.len >= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-7",
        phase: Phase::Registration,
        summary: "Output names unique",
        predicate: "unique(outputs[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-8",
        phase: Phase::Registration,
        summary: "Side effects not allowed",
        predicate: "side_effects == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-9",
        phase: Phase::Registration,
        summary: "State resettable if allowed",
        predicate: "state.allowed => state.resettable",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-10",
        phase: Phase::Registration,
        summary: "Errors deterministic",
        predicate: "errors.allowed => errors.deterministic",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-11",
        phase: Phase::Execution,
        summary: "All outputs produced on success",
        predicate: "compute() -> Ok(outputs) includes every declared output",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-12",
        phase: Phase::Execution,
        summary: "No outputs produced on error",
        predicate: "compute() -> Err(_) emits no outputs",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-13",
        phase: Phase::Registration,
        summary: "Input types valid",
        predicate: "inputs[].type ∈ {Number, Bool, Series}",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-14",
        phase: Phase::Registration,
        summary: "Input cardinality single",
        predicate: "inputs[].cardinality == single",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-15",
        phase: Phase::Registration,
        summary: "Parameter types valid",
        predicate: "parameters[].type ∈ {Int, Number, Bool}",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-16",
        phase: Phase::Registration,
        summary: "Cadence is continuous",
        predicate: "execution.cadence == continuous",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-17",
        phase: Phase::Registration,
        summary: "Execution deterministic",
        predicate: "execution.deterministic == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-18",
        phase: Phase::Registration,
        summary: "ID unique in registry",
        predicate: "!registry.contains_key(id)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-19",
        phase: Phase::Registration,
        summary: "Parameter default type matches declared type",
        predicate:
            "parameters[].default == None || typeof(parameters[].default) == parameters[].type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "CMP-20",
        phase: Phase::Registration,
        summary: "Output types valid",
        predicate: "all(outputs[].type ∈ ValueType)",
        status: RuleStatus::Enforced,
    },
];

const TRIGGER_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "TRG-1",
        phase: Phase::Registration,
        summary: "ID format valid",
        predicate: "regex(id, /^[a-z][a-z0-9_]*$/)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-2",
        phase: Phase::Registration,
        summary: "Version valid semver",
        predicate: "semver.valid(version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-3",
        phase: Phase::Registration,
        summary: "Kind is \"trigger\"",
        predicate: "kind == \"trigger\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-4",
        phase: Phase::Registration,
        summary: "At least one input",
        predicate: "inputs.len >= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-5",
        phase: Phase::Registration,
        summary: "Input names unique",
        predicate: "unique(inputs[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-6",
        phase: Phase::Registration,
        summary: "Input types valid",
        predicate: "all(inputs[].type ∈ TriggerValueType)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-7",
        phase: Phase::Registration,
        summary: "Exactly one output",
        predicate: "outputs.len == 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-8",
        phase: Phase::Registration,
        summary: "Output is event type",
        predicate: "outputs[0].type == \"event\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-9",
        phase: Phase::Registration,
        summary: "State not allowed",
        predicate: "state.allowed == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-10",
        phase: Phase::Registration,
        summary: "Side effects not allowed",
        predicate: "side_effects == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-11",
        phase: Phase::Registration,
        summary: "Execution deterministic",
        predicate: "execution.deterministic == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-12",
        phase: Phase::Registration,
        summary: "Input cardinality single",
        predicate: "inputs[].cardinality == single",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-13",
        phase: Phase::Registration,
        summary: "ID unique in registry",
        predicate: "!registry.contains_key(id)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "TRG-14",
        phase: Phase::Registration,
        summary: "Parameter default type matches declared type",
        predicate:
            "parameters[].default == None || typeof(parameters[].default) == parameters[].type",
        status: RuleStatus::Enforced,
    },
];

const ACTION_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "ACT-1",
        phase: Phase::Registration,
        summary: "ID format valid",
        predicate: "regex(id, /^[a-z][a-z0-9_]*$/)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-2",
        phase: Phase::Registration,
        summary: "Version valid semver",
        predicate: "semver.valid(version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-3",
        phase: Phase::Registration,
        summary: "Kind is \"action\"",
        predicate: "kind == \"action\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-4",
        phase: Phase::Registration,
        summary: "At least one event input",
        predicate: "any(inputs[].type == \"event\")",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-5",
        phase: Phase::Registration,
        summary: "Input names unique",
        predicate: "unique(inputs[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-6",
        phase: Phase::Registration,
        summary: "Input types valid",
        predicate: "all(inputs[].type ∈ ActionValueType)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-7",
        phase: Phase::Registration,
        summary: "Exactly one output",
        predicate: "outputs.len == 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-8",
        phase: Phase::Registration,
        summary: "Output named \"outcome\"",
        predicate: "outputs[0].name == \"outcome\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-9",
        phase: Phase::Registration,
        summary: "Output is event type",
        predicate: "outputs[0].type == \"event\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-10",
        phase: Phase::Registration,
        summary: "State not allowed",
        predicate: "state.allowed == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-11",
        phase: Phase::Registration,
        summary: "Side effects required",
        predicate: "side_effects == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-12",
        phase: Phase::Composition,
        summary: "Gated by trigger",
        predicate: "validation-phase alias of V.5 (R.7 is separate execution-time skip behavior)",
        status: RuleStatus::Alias { enforced_by: "V.5" },
    },
    RuleDefinition {
        id: "ACT-13",
        phase: Phase::Registration,
        summary: "Effects surface normalized",
        predicate:
            "file-backed parse defaults omitted effects to empty writes; runtime manifest always carries ActionEffects",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-14",
        phase: Phase::Registration,
        summary: "Write names unique",
        predicate: "unique(effects.writes[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-15",
        phase: Phase::Registration,
        summary: "Write types valid",
        predicate: "all(effects.writes[].type ∈ {Number, Series, Bool, String})",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-16",
        phase: Phase::Registration,
        summary: "Retryable false",
        predicate: "execution.retryable == false",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-17",
        phase: Phase::Registration,
        summary: "Execution deterministic",
        predicate: "execution.deterministic == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-18",
        phase: Phase::Registration,
        summary: "ID unique in registry",
        predicate: "!registry.contains_key(id)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-19",
        phase: Phase::Registration,
        summary: "Parameter default type matches declared type",
        predicate:
            "parameters[].default == None || typeof(parameters[].default) == parameters[].type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-20",
        phase: Phase::Registration,
        summary: "$key write references bound to declared parameter",
        predicate: "∀ write where name starts with \"$\": referenced param exists",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-21",
        phase: Phase::Registration,
        summary: "$key write references must be String type",
        predicate: "∀ write where name starts with \"$\": referenced param.type == String",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-22",
        phase: Phase::Registration,
        summary: "Write from_input references declared input",
        predicate: "∀ write: from_input ∈ inputs[].name",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-23",
        phase: Phase::Registration,
        summary: "Write from_input type compatible with write type",
        predicate: "∀ write: inputs[from_input].type is scalar AND matches write.value_type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-24",
        phase: Phase::Registration,
        summary: "Intent names unique",
        predicate: "unique(effects.intents[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-25",
        phase: Phase::Registration,
        summary: "Intent field names unique within each intent",
        predicate: "∀ intent: unique(intent.fields[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-26",
        phase: Phase::Registration,
        summary: "Intent field declares a source",
        predicate: "∀ field: field.from_input != None OR field.from_param != None",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-27",
        phase: Phase::Registration,
        summary: "Intent field declares only one source",
        predicate: "∀ field: !(field.from_input != None AND field.from_param != None)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-28",
        phase: Phase::Registration,
        summary: "Intent field from_input references declared input",
        predicate: "∀ field where from_input != None: from_input ∈ inputs[].name",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-29",
        phase: Phase::Registration,
        summary: "Intent field from_input type compatible with field type",
        predicate: "∀ field where from_input != None: inputs[from_input].type is scalar AND matches field.value_type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-30",
        phase: Phase::Registration,
        summary: "Intent field from_param references declared parameter",
        predicate: "∀ field where from_param != None: from_param ∈ parameters[].name",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-31",
        phase: Phase::Registration,
        summary: "Intent field from_param type compatible with field type",
        predicate: "∀ field where from_param != None: parameters[from_param].type matches field.value_type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-32",
        phase: Phase::Registration,
        summary: "Mirror write from_field references declared intent field",
        predicate: "∀ mirror_write: from_field ∈ intent.fields[].name",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "ACT-33",
        phase: Phase::Registration,
        summary: "Mirror write type matches referenced field type",
        predicate: "∀ mirror_write: mirror_write.value_type == intent.fields[from_field].value_type",
        status: RuleStatus::Enforced,
    },
];

const COMPOSITION_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "COMP-1",
        phase: Phase::Composition,
        summary: "Source context requirements satisfied",
        predicate: "source.requires.context.filter(required).keys ⊆ adapter.provides.context.keys",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-2",
        phase: Phase::Composition,
        summary: "Source context types match",
        predicate:
            "∀ k in source.requires.context where adapter provides k: source.requires.context[k].ty == adapter.provides.context[k].ty",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-3",
        phase: Phase::Composition,
        summary: "Capture format version supported",
        predicate: "runtime.supports_capture(adapter.capture.format_version)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-4",
        phase: Phase::Composition,
        summary: "Source output type equals Compute input type",
        predicate: "source.output.type == compute.input.type",
        status: RuleStatus::Alias { enforced_by: "V.4" },
    },
    RuleDefinition {
        id: "COMP-5",
        phase: Phase::Composition,
        summary: "Input type equals upstream output type",
        predicate: "upstream.output.type == compute.input.type",
        status: RuleStatus::Alias { enforced_by: "V.4" },
    },
    RuleDefinition {
        id: "COMP-6",
        phase: Phase::Composition,
        summary: "Output type equals downstream input type",
        predicate: "compute.output.type == downstream.input.type",
        status: RuleStatus::Alias { enforced_by: "V.4" },
    },
    RuleDefinition {
        id: "COMP-7",
        phase: Phase::Composition,
        summary: "Trigger input from Compute or Trigger",
        predicate: "upstream.kind ∈ {\"compute\", \"trigger\"}",
        status: RuleStatus::Alias { enforced_by: "V.2" },
    },
    RuleDefinition {
        id: "COMP-8",
        phase: Phase::Composition,
        summary: "Trigger output to Action or Trigger",
        predicate: "downstream.kind ∈ {\"action\", \"trigger\"}",
        status: RuleStatus::Alias { enforced_by: "V.2" },
    },
    RuleDefinition {
        id: "COMP-9",
        phase: Phase::Composition,
        summary: "Action inputs follow gate/payload split",
        predicate: "∀ action input i: (i.type == event => upstream(i).kind == \"trigger\") ∧ (i.type != event => upstream(i).kind ∈ {\"source\",\"compute\"})",
        status: RuleStatus::Alias { enforced_by: "V.2" },
    },
    RuleDefinition {
        id: "COMP-10",
        phase: Phase::Composition,
        summary: "Action output not wireable",
        predicate: "downstream.len == 0",
        status: RuleStatus::Alias { enforced_by: "V.2" },
    },
    RuleDefinition {
        id: "COMP-11",
        phase: Phase::Composition,
        summary: "Action writes target provided keys",
        predicate: "effects.writes.names ⊆ adapter.context_keys.names",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-12",
        phase: Phase::Composition,
        summary: "Action writes only writable keys",
        predicate: "∀n ∈ writes: adapter.key[n].writable == true",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-13",
        phase: Phase::Composition,
        summary: "Action write types match",
        predicate: "∀n ∈ writes: action.type[n] == adapter.key[n].type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-14",
        phase: Phase::Composition,
        summary: "If action writes or mirror writes, adapter accepts set_context",
        predicate: "(writes.len > 0 OR any(intent.mirror_writes.len > 0)) => accepts.effects contains \"set_context\"",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-15",
        phase: Phase::Composition,
        summary: "Writes captured (planned)",
        predicate:
            "writes.len > 0 => (\"effect.set_context\" ∈ capture.fields AND ∀n: \"context.\" + n ∈ capture.fields)",
        status: RuleStatus::Deferred,
    },
    RuleDefinition {
        id: "COMP-16",
        phase: Phase::Composition,
        summary: "Parameter-bound manifest names resolve",
        predicate: "$-prefixed names resolve to String parameter values",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-17",
        phase: Phase::Composition,
        summary: "If action declares intents, adapter accepts each intent effect kind",
        predicate: "effects.intents.names ⊆ accepts.effects.names",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-18",
        phase: Phase::Composition,
        summary: "Declared intent kinds must have payload schemas in adapter acceptance surface",
        predicate: "∀ intent: accepts.effects[intent.name].payload_schema exists",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "COMP-19",
        phase: Phase::Composition,
        summary: "Intent fields are structurally compatible with adapter payload schema",
        predicate: "∀ intent: intent.fields structurally compatible with accepts.effects[intent.name].payload_schema",
        status: RuleStatus::Enforced,
    },
];

const CLUSTER_RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "D.1",
        phase: Phase::Composition,
        summary: "Cluster contains >= 1 node",
        predicate: "cluster.nodes.len >= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.2",
        phase: Phase::Composition,
        summary: "Edges reference existing nodes/ports",
        predicate: "edge endpoints refer to existing nodes/ports",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.3",
        phase: Phase::Composition,
        summary: "Edges satisfy wiring matrix",
        predicate: "wiring_allowed(from.kind, to.kind)",
        status: RuleStatus::Alias { enforced_by: "V.2" },
    },
    RuleDefinition {
        id: "D.4",
        phase: Phase::Composition,
        summary: "Output ports reference valid internal node outputs",
        predicate: "output_ports[].maps_to refer to existing node outputs",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.5",
        phase: Phase::Composition,
        summary: "Input port names unique",
        predicate: "unique(input_ports[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.6",
        phase: Phase::Composition,
        summary: "Output port names unique",
        predicate: "unique(output_ports[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.8",
        phase: Phase::Composition,
        summary: "Parameter defaults type-compatible",
        predicate: "parameter.default matches parameter.ty",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.9",
        phase: Phase::Composition,
        summary: "No duplicate parameter names",
        predicate: "unique(parameters[].name)",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.10",
        phase: Phase::Composition,
        summary: "Declared wireability compatible with inferred",
        predicate: "declared.wireable <= inferred.wireable",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "D.11",
        phase: Phase::Composition,
        summary: "Declared wireability <= inferred",
        predicate: "declared.wireable <= inferred.wireable",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "I.3",
        phase: Phase::Composition,
        summary: "Required parameters bound or exposed",
        predicate: "all required cluster parameters are bound or exposed at instantiation",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "I.4",
        phase: Phase::Composition,
        summary: "Bound parameter values type-compatible",
        predicate: "bound and exposed parameter values match declared parameter types",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "I.5",
        phase: Phase::Composition,
        summary: "Exposed parameters exist in parent",
        predicate: "every exposed parameter exists in the parent context",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "I.6",
        phase: Phase::Composition,
        summary: "Version constraints satisfied",
        predicate: "cluster and primitive selectors resolve to an available satisfying version",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "I.7",
        phase: Phase::Composition,
        summary: "Parameter bindings reference only declared parameters",
        predicate: "binding names are members of the target parameter set",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "E.3",
        phase: Phase::Composition,
        summary: "ExternalInput not an edge sink",
        predicate: "no ExpandedEndpoint::ExternalInput in expanded.edges",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "E.9",
        phase: Phase::Composition,
        summary: "Referenced nested clusters exist",
        predicate: "every NodeKind::Cluster reference resolves through the cluster loader",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.1",
        phase: Phase::Composition,
        summary: "No cycles in graph",
        predicate: "topological_sort succeeds",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.2",
        phase: Phase::Composition,
        summary: "Edges satisfy wiring matrix (including Action gate/payload input refinement)",
        predicate: "wiring_allowed(from.kind, to.kind) OR (to.kind == action AND to.input.type ∈ {number,bool,string,series} AND from.kind ∈ {source,compute})",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.3",
        phase: Phase::Composition,
        summary: "Required inputs connected",
        predicate: "all required inputs have inbound edge",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.4",
        phase: Phase::Composition,
        summary: "Type constraints satisfied at edges",
        predicate: "edge output type == input expected type",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.5",
        phase: Phase::Composition,
        summary: "Actions gated by triggers",
        predicate: "every action has inbound trigger event edge",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.7",
        phase: Phase::Composition,
        summary: "Each input has <= 1 inbound edge",
        predicate: "inbound_edges(input) <= 1",
        status: RuleStatus::Enforced,
    },
    RuleDefinition {
        id: "V.8",
        phase: Phase::Composition,
        summary: "Referenced primitive implementations exist in catalog",
        predicate: "every expanded node resolves through the primitive catalog",
        status: RuleStatus::Enforced,
    },
];

#[cfg(test)]
mod tests {
    use super::generate_rule_registry_markdown;

    #[test]
    fn generated_registry_includes_act_24_through_act_33() {
        let rendered = generate_rule_registry_markdown();
        for id in 24..=33 {
            assert!(
                rendered.contains(&format!("| ACT-{id} |")),
                "missing ACT-{id} in generated registry"
            );
        }
    }

    #[test]
    fn generated_registry_includes_comp_17_through_comp_19() {
        let rendered = generate_rule_registry_markdown();
        for id in 17..=19 {
            assert!(
                rendered.contains(&format!("| COMP-{id} |")),
                "missing COMP-{id} in generated registry"
            );
        }
    }
}
