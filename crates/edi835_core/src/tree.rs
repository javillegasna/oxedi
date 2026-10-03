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
    /// `true` when the engine opened it without a triggering segment.
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

impl LoopTree {
    /// Runs the engine over `segments` and folds its events into a tree.
    pub fn build<'a>(spec: &Spec, segments: impl IntoIterator<Item = Segment<'a>>) -> LoopTree {
        let mut tree = LoopTree {
            nodes: vec![Node::root()],
        };
        let mut open = vec![NodeId(0)];
        let mut engine = LoopEngine::new(spec);
        for segment in segments {
            for &event in engine.feed(&segment) {
                tree.apply(&mut open, event);
            }
        }
        for &event in engine.finish() {
            tree.apply(&mut open, event);
        }
        tree
    }

    fn apply(&mut self, open: &mut Vec<NodeId>, event: Event) {
        let current = *open.last().unwrap_or(&NodeId(0));
        match event {
            Event::LoopOpened {
                id,
                implicit,
                segment,
            } => {
                let node = NodeId(self.nodes.len());
                self.nodes.push(Node {
                    loop_id: Some(id),
                    implicit,
                    opened_by: Some(segment),
                    parent: Some(current),
                    children: Vec::new(),
                    segments: Vec::new(),
                    unmatched: Vec::new(),
                });
                self.nodes[current.0].children.push(node);
                open.push(node);
            }
            Event::LoopClosed { .. } => {
                if open.len() > 1 {
                    open.pop();
                }
            }
            Event::Captured { segment, .. } => self.nodes[current.0].segments.push(segment),
            Event::Unmatched { segment } => self.nodes[current.0].unmatched.push(segment),
            Event::Empty { .. } => {}
        }
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
mod tests {
    use super::*;
    use crate::{Delimiters, Spec, Tokenizer};

    const TINY: &str = r#"{"name":"t","loops":{
        "A":{"trigger":{"segment":"AA"},"segments":["A1"],"end":"AE"},
        "B":{"parent":"A","trigger":{"segment":"BB"},"segments":["B1"]}
    }}"#;

    fn tree(input: &[u8]) -> (Spec, LoopTree) {
        let spec = Spec::from_json(TINY).unwrap();
        let tree = LoopTree::build(
            &spec,
            Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')),
        );
        (spec, tree)
    }

    #[test]
    fn root_holds_top_level_loops() {
        let (spec, tree) = tree(b"AA~A1~BB~B1~BB~AE~AA~");
        let root = tree.node(tree.root());
        assert_eq!(root.loop_id, None);
        assert_eq!(root.children.len(), 2, "two A instances");
        let a = tree.node(root.children[0]);
        assert_eq!(a.loop_id, spec.loop_id("A"));
        assert_eq!(a.segments, vec![0, 1, 5], "AA, A1 and the end AE");
        assert_eq!(a.children.len(), 2, "two B instances");
        assert_eq!(tree.node(a.children[0]).segments, vec![2, 3]);
        assert_eq!(tree.node(a.children[1]).segments, vec![4]);
        assert_eq!(tree.node(a.children[1]).parent, Some(root.children[0]));
        assert_eq!(tree.node_count(), 5);
    }

    #[test]
    fn unmatched_segments_hang_from_the_current_node_and_empty_ones_vanish() {
        let (spec, tree) = tree(b"ZZ~AA~BB~ZZ~\n");
        let root = tree.node(tree.root());
        assert_eq!(root.unmatched, vec![0]);
        let b = tree.nodes_of(spec.loop_id("B").unwrap()).next().unwrap();
        assert_eq!(tree.node(b).unmatched, vec![3]);
        let listed: usize = tree
            .nodes()
            .iter()
            .map(|n| n.segments.len() + n.unmatched.len())
            .sum();
        assert_eq!(listed, 4, "the trailing empty segment is not in the tree");
    }

    #[test]
    fn implicit_nodes_are_flagged() {
        let (spec, tree) = tree(b"BB~");
        let a = tree.nodes_of(spec.loop_id("A").unwrap()).next().unwrap();
        assert!(tree.node(a).implicit);
        let b = tree.nodes_of(spec.loop_id("B").unwrap()).next().unwrap();
        assert!(!tree.node(b).implicit);
        assert!(tree.node(a).segments.is_empty());
    }

    #[test]
    fn every_node_but_the_root_knows_the_segment_that_opened_it() {
        let (spec, explicit) = tree(b"AA~A1~BB~B1~BB~AE~AA~");
        assert_eq!(explicit.node(explicit.root()).opened_by, None);
        let opened: Vec<(&str, Option<usize>)> = explicit.nodes()[1..]
            .iter()
            .map(|node| {
                (
                    node.loop_id.map_or("", |id| spec.loop_name(id)),
                    node.opened_by,
                )
            })
            .collect();
        assert_eq!(
            opened,
            vec![
                ("A", Some(0)),
                ("B", Some(2)),
                ("B", Some(4)),
                ("A", Some(6))
            ]
        );
        let (spec, implicit) = tree(b"BB~");
        let a = implicit
            .nodes_of(spec.loop_id("A").unwrap())
            .next()
            .unwrap();
        assert_eq!(
            implicit.node(a).opened_by,
            Some(0),
            "the implicit A was opened for BB"
        );
    }

    #[test]
    fn empty_input_is_a_lone_root() {
        let (_, tree) = tree(b"");
        assert_eq!(tree.node_count(), 1);
        assert!(tree.node(tree.root()).children.is_empty());
    }
}
