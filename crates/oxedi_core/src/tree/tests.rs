use super::*;
use crate::{Delimiters, Spec, Tokenizer};

const TINY: &str = r#"{"name":"t","loops":{
    "A":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0},"a1":{"segment":"A1","pos":1}},"end":"AE"},
    "B":{"parent":"A","trigger":{"segment":"BB"},"occurrences":{"bb":{"segment":"BB","pos":0},"b1":{"segment":"B1","pos":1}}}
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
fn a_builder_folds_hand_built_events_in_slices() {
    let spec = Spec::from_json(TINY).unwrap();
    let a = spec.loop_id("A").unwrap();
    let b = spec.loop_id("B").unwrap();
    let mut builder = LoopTree::builder();
    builder.on(&[
        Event::LoopOpened {
            id: a,
            implicit: false,
            segment: 0,
        },
        Event::Captured { id: a, segment: 0 },
    ]);
    builder.on(&[]);
    builder.on(&[
        Event::LoopOpened {
            id: b,
            implicit: true,
            segment: 1,
        },
        Event::Captured { id: b, segment: 1 },
        Event::Unmatched { segment: 2 },
        Event::Empty { segment: 3 },
        Event::LoopClosed { id: b },
        Event::LoopClosed { id: a },
        Event::LoopClosed { id: a },
        Event::Unmatched { segment: 4 },
    ]);
    let tree = builder.finish();
    assert_eq!(tree.node_count(), 3);
    let root = tree.node(tree.root());
    assert_eq!(root.unmatched, vec![4], "extra closes never pop the root");
    let a_node = tree.node(root.children[0]);
    assert_eq!(a_node.segments, vec![0]);
    let b_node = tree.node(a_node.children[0]);
    assert!(b_node.implicit);
    assert_eq!(b_node.segments, vec![1]);
    assert_eq!(b_node.unmatched, vec![2]);
}

#[test]
fn empty_input_is_a_lone_root() {
    let (_, tree) = tree(b"");
    assert_eq!(tree.node_count(), 1);
    assert!(tree.node(tree.root()).children.is_empty());
}
