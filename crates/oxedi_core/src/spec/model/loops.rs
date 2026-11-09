//! The loops of a spec and their occurrences, as the spec writes them.

use serde::{Deserialize, Serialize};

/// The element (or component) and the codes that tell an occurrence apart
/// from the other occurrences of its segment in the same loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a qualifier object")]
pub struct QualifierModel {
    /// 1-based element position.
    pub element: usize,
    /// 1-based component position inside the element, when the qualifier is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<usize>,
    /// The codes the element holds in this occurrence.
    pub codes: Vec<String>,
}
