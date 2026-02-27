pub mod r#impl;
pub mod manifest;

pub use manifest::emit_if_event_and_true_manifest;
pub use r#impl::EmitIfEventAndTrue;
