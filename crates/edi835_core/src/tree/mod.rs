//! A tree of loop instances built from the engine's events.
//!
//! Nodes live in one vector and refer to each other by index. The root (node 0)
//! is virtual: it has no loop and holds the top-level instances. Captured and
//! unmatched segments are stored as indices into the stream they came from.

use crate::engine::{Event, LoopEngine};
use crate::segment::Segment;
use crate::spec::{LoopId, Spec};

/// Index of a node inside a [`LoopTree`]. Only a tree creates these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

impl NodeId {
    /// Position of the node in [`LoopTree::nodes`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// One loop instance (or the virtual root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The loop, or `None` for the root.
    pub loop_id: Option<LoopId>,
    /// `true` when no segment of its own opened it.
    pub implicit: bool,
    /// Index of the trigger segment that caused the opening (for an implicit
    /// node, the trigger of the descendant that needed it); `None` only for
    /// the root.
    pub opened_by: Option<usize>,
    /// The enclosing node, or `None` for the root.
    pub parent: Option<NodeId>,
    /// Nested instances, in stream order.
    pub children: Vec<NodeId>,
    /// Indices of the segments captured here, in stream order.
    pub segments: Vec<usize>,
    /// Indices of the segments no loop could hold while this node was current.
    pub unmatched: Vec<usize>,
}

/// Loop instances of one stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopTree {
    nodes: Vec<Node>,
}

/// Folds engine events into a [`LoopTree`] as the caller receives them.
///
/// For a caller that drives its own [`LoopEngine`]: pass every slice `feed` and
/// `finish` return to [`on`](TreeBuilder::on), in order.
#[derive(Debug, Clone)]
pub struct TreeBuilder {
    tree: LoopTree,
    open: Vec<NodeId>,
}

impl TreeBuilder {
    /// Folds `events`, in the order the engine returned them.
    pub fn on(&mut self, events: &[Event]) {
        for &event in events {
            self.apply(event);
        }
    }

    /// The tree of everything folded so far.
    pub fn finish(self) -> LoopTree {
        self.tree
    }

    fn apply(&mut self, event: Event) {
        let current = *self.open.last().unwrap_or(&NodeId(0));
        let tree = &mut self.tree;
        match event {
            Event::LoopOpened {
                id,
                implicit,
                segment,
            } => {
                let node = NodeId(tree.nodes.len());
                tree.nodes.push(Node {
                    loop_id: Some(id),
                    implicit,
                    opened_by: Some(segment),
                    parent: Some(current),
                    children: Vec::new(),
                    segments: Vec::new(),
                    unmatched: Vec::new(),
                });
                tree.nodes[current.0].children.push(node);
                self.open.push(node);
            }
            Event::LoopClosed { .. } => {
                if self.open.len() > 1 {
                    self.open.pop();
                }
            }
            Event::Captured { segment, .. } => tree.nodes[current.0].segments.push(segment),
            Event::Unmatched { segment } => tree.nodes[current.0].unmatched.push(segment),
            Event::Empty { .. } => {}
        }
    }
}

impl LoopTree {
    /// An empty builder, for events the caller gets from its own engine.
    pub fn builder() -> TreeBuilder {
        TreeBuilder {
            tree: LoopTree {
                nodes: vec![Node::root()],
            },
            open: vec![NodeId(0)],
        }
    }

    /// Runs the engine over `segments` and folds its events into a tree.
    pub fn build<'a>(spec: &Spec, segments: impl IntoIterator<Item = Segment<'a>>) -> LoopTree {
        let mut builder = LoopTree::builder();
        let mut engine = LoopEngine::new(spec);
        for segment in segments {
            builder.on(engine.feed(&segment));
        }
        builder.on(engine.finish());
        builder.finish()
    }

    /// The virtual root.
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// A node by id.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    /// Every node, root first, in creation (stream) order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Number of nodes, root included.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Instances of one loop, in stream order.
    pub fn nodes_of(&self, id: LoopId) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| node.loop_id == Some(id))
            .map(|(index, _)| NodeId(index))
    }
}

impl Node {
    fn root() -> Node {
        Node {
            loop_id: None,
            implicit: false,
            opened_by: None,
            parent: None,
            children: Vec::new(),
            segments: Vec::new(),
            unmatched: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
