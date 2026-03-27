pub fn doc_anchor_for_rule(rule_id: &str) -> &'static str {
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
        "NUM-FINITE-1" => "docs/invariants/00-cross-phase.md",
        id if id.starts_with("X.") => "docs/invariants/00-cross-phase.md",
        "GW-EFX-META-1" => "docs/contracts/ui-runtime.md#3-metadata-requirement-for-intent-effects",
        _ => "docs/invariants/INDEX.md",
    }
}

#[cfg(test)]
mod tests {
    use super::doc_anchor_for_rule;

    #[test]
    fn maps_current_doc_families() {
        assert_eq!(
            doc_anchor_for_rule("ADP-7"),
            "docs/primitives/adapter.md#43-enforcement-mapping"
        );
        assert_eq!(
            doc_anchor_for_rule("COMP-1"),
            "docs/primitives/adapter.md#51-composition-enforcement-mapping"
        );
        assert_eq!(
            doc_anchor_for_rule("V.8"),
            "docs/authoring/cluster-spec.md#64-enforcement-mapping-phase-6"
        );
        assert_eq!(
            doc_anchor_for_rule("X.11"),
            "docs/invariants/00-cross-phase.md"
        );
        assert_eq!(
            doc_anchor_for_rule("GW-EFX-META-1"),
            "docs/contracts/ui-runtime.md#3-metadata-requirement-for-intent-effects"
        );
    }
}
