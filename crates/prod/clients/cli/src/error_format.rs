use serde_json::{json, Value as JsonValue};

#[derive(Debug, Clone)]
pub struct CliErrorInfo {
    pub code: String,
    pub message: String,
    pub rule_id: Option<String>,
    pub where_field: Option<String>,
    pub fix: Option<String>,
    pub details: Vec<String>,
}

impl CliErrorInfo {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            rule_id: None,
            where_field: None,
            fix: None,
            details: Vec::new(),
        }
    }

    pub fn with_rule_id(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self
    }

    pub fn with_where(mut self, where_field: impl Into<String>) -> Self {
        self.where_field = Some(where_field.into());
        self
    }

    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }
}

pub fn render_cli_error(info: &CliErrorInfo) -> String {
    let mut lines = vec![
        format!("error: {}", info.message),
        format!("code: {}", info.code),
    ];

    if let Some(rule_id) = &info.rule_id {
        lines.push(format!("rule: {rule_id}"));
    }
    if let Some(where_field) = &info.where_field {
        lines.push(format!("where: {where_field}"));
    }
    if let Some(fix) = &info.fix {
        lines.push(format!("fix: {fix}"));
    }
    for detail in &info.details {
        lines.push(format!("detail: {detail}"));
    }

    lines.join("\n")
}

#[allow(dead_code)]
pub fn render_cli_error_json(info: &CliErrorInfo) -> JsonValue {
    json!({
        "code": info.code,
        "message": info.message,
        "rule_id": info.rule_id,
        "where": info.where_field,
        "fix": info.fix,
        "details": info.details,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_error_renders_text_shape() {
        let rendered = render_cli_error(
            &CliErrorInfo::new("cli.invalid_option", "invalid option")
                .with_rule_id("RUN-CANON-1")
                .with_where("arg '--direct'")
                .with_fix("remove --direct")
                .with_detail("detail one"),
        );
        assert!(rendered.contains("error: invalid option"));
        assert!(rendered.contains("code: cli.invalid_option"));
        assert!(rendered.contains("rule: RUN-CANON-1"));
        assert!(rendered.contains("where: arg '--direct'"));
        assert!(rendered.contains("fix: remove --direct"));
        assert!(rendered.contains("detail: detail one"));
    }

    #[test]
    fn cli_error_renders_json_shape() {
        let rendered = render_cli_error_json(
            &CliErrorInfo::new("cli.invalid_option", "invalid option").with_where("arg '--direct'"),
        );
        assert_eq!(rendered["code"], "cli.invalid_option");
        assert_eq!(rendered["message"], "invalid option");
        assert_eq!(rendered["where"], "arg '--direct'");
    }
}
