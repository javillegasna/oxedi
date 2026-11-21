//! A tree of loop instances built from the engine's events.
//!
//! Nodes live in one vector and refer to each other by index. The root (node 0)
//! is virtual: it has no loop and holds the top-level instances. Captured and
//! unmatched segments are stored as indices into the stream they came from.
//!
//! The builder finds a node by the instance id an event carries: a table maps
//! each opened instance's id to its node, and an id the builder never saw
//! opened resolves to the root. A later instance with the same id replaces the
//! entry, so a builder fed several streams in a row keeps every node and hangs
//! each stream's top-level instances from the root.

use crate::engine::{Event, InstanceId, LoopEngine};
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
    /// Indices of the segments no loop could hold, reported within this
    /// instance (the innermost open one when they were read); the root holds
    /// those reported at the top level or within an instance never opened.
    pub unmatched: Vec<usize>,
}

/// Loop instances of the streams folded into it.
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
    /// The node of each opened instance, indexed by [`InstanceId::index`].
    by_instance: Vec<Option<NodeId>>,
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

    /// The node of an instance; the root for `None` or an id never opened.
    fn node_of(&self, id: Option<InstanceId>) -> NodeId {
        id.and_then(|id| self.by_instance.get(id.index()))
            .copied()
            .flatten()
            .unwrap_or(NodeId(0))
    }

    /// Records `node` as the node of instance `id`, replacing an older entry.
    fn bind(&mut self, id: InstanceId, node: NodeId) {
        if self.by_instance.len() <= id.index() {
            self.by_instance.resize(id.index() + 1, None);
        }
        self.by_instance[id.index()] = Some(node);
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::LoopOpened {
                instance,
                implicit,
                segment,
            } => {
                let parent = self.node_of(instance.parent);
                let node = NodeId(self.tree.nodes.len());
                self.tree.nodes.push(Node {
                    loop_id: Some(instance.loop_id),
                    implicit,
                    opened_by: Some(segment),
                    parent: Some(parent),
                    children: Vec::new(),
                    segments: Vec::new(),
                    unmatched: Vec::new(),
                });
                self.tree.nodes[parent.0].children.push(node);
                self.bind(instance.id, node);
            }
            Event::Captured {
                instance, segment, ..
            } => {
                let node = self.node_of(Some(instance.id));
                self.tree.nodes[node.0].segments.push(segment);
            }
            Event::Unmatched { segment, within } => {
                let node = self.node_of(within.map(|instance| instance.id));
                self.tree.nodes[node.0].unmatched.push(segment);
            }
            Event::LoopClosed { .. } | Event::Empty { .. } => {}
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
            by_instance: Vec::new(),
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
