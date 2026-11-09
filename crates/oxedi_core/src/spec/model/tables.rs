//! The tables of a spec, as the spec writes them.

use serde::{Deserialize, Serialize};

/// Where the repeated elements of a table's segment start and how far apart
/// they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a repeat object")]
pub struct RepeatModel {
    /// 1-based element position of the first repetition.
    pub from: usize,
    /// Elements between one repetition and the next.
    pub step: usize,
}
