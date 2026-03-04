use std::borrow::Cow;

pub trait ErrorInfo {
    fn rule_id(&self) -> &'static str;
    fn phase(&self) -> Phase;
    fn doc_anchor(&self) -> &'static str;
    fn summary(&self) -> Cow<'static, str>;
    fn path(&self) -> Option<Cow<'static, str>>;
    fn fix(&self) -> Option<Cow<'static, str>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Registration,
    Composition,
    Execution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleViolation {
    pub rule_id: &'static str,
    pub phase: Phase,
    pub doc_anchor: &'static str,
    pub summary: Cow<'static, str>,
    pub path: Option<Cow<'static, str>>,
    pub fix: Option<Cow<'static, str>>,
}

impl<E: ErrorInfo> From<E> for RuleViolation {
    fn from(e: E) -> Self {
        RuleViolation {
            rule_id: e.rule_id(),
            phase: e.phase(),
            doc_anchor: e.doc_anchor(),
            summary: e.summary(),
            path: e.path(),
            fix: e.fix(),
        }
    }
}

impl RuleViolation {
    pub fn from_ref(e: &dyn ErrorInfo) -> Self {
        RuleViolation {
            rule_id: e.rule_id(),
            phase: e.phase(),
            doc_anchor: e.doc_anchor(),
            summary: e.summary(),
            path: e.path(),
            fix: e.fix(),
        }
    }
}
