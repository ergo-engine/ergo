use ergo_runtime::common::ErrorInfo;

pub fn render_error_info(err: &impl ErrorInfo) -> String {
    let mut msg = format!("[{}] {}", err.rule_id(), err.summary());
    if let Some(path) = err.path() {
        msg.push_str(&format!(" (path: {path})"));
    }
    if let Some(fix) = err.fix() {
        msg.push_str(&format!("; fix: {fix}"));
    }
    msg
}
