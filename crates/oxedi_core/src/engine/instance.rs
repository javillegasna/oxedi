//! One occurrence of a loop in the stream, numbered as the engine opens it.

use crate::spec::LoopId;

/// Identifies an [`Instance`] by its opening order in the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(usize);

impl InstanceId {
    /// The id of the `index`th instance the engine opened (0-based).
    pub(super) fn new(index: usize) -> Self {
        Self(index)
    }

    /// 0-based opening order of the instance in the stream.
    pub fn index(self) -> usize {
        self.0
    }
}

/// An open occurrence of a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instance {
    /// This occurrence.
    pub id: InstanceId,
    /// The loop it is an occurrence of.
    pub loop_id: LoopId,
    /// 1-based position among the occurrences of `loop_id` over the whole
    /// stream, whatever the parent.
    pub ordinal_in_stream: usize,
    /// The enclosing occurrence; `None` at the top level.
    pub parent: Option<InstanceId>,
}
