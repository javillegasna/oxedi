use super::*;
use crate::spec::test_support::nested;
use crate::{Delimiters, Instance, InstanceId, Spec, Tokenizer};

fn tree(input: &[u8]) -> (Spec, LoopTree) {
    let spec = nested();
    let tree = LoopTree::build(
        &spec,
        Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')),
    );
    (spec, tree)
}

/// Feeds one stream through a fresh engine into `builder`.
fn fold(builder: &mut TreeBuilder, spec: &Spec, input: &[u8]) {
    let mut engine = LoopEngine::new(spec);
    for segment in Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')) {
        builder.on(engine.feed(&segment));
    }
    builder.on(engine.finish());
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
    let spec = nested();
    let a_id = spec.loop_id("A").unwrap();
    let b_id = spec.loop_id("B").unwrap();
    let a = Instance {
        id: InstanceId::at(0),
        loop_id: a_id,
        ordinal_in_stream: 1,
        parent: None,
    };
    let b = Instance {
        id: InstanceId::at(1),
        loop_id: b_id,
        ordinal_in_stream: 1,
        parent: Some(a.id),
    };
    let mut builder = LoopTree::builder();
    builder.on(&[
        Event::LoopOpened {
            instance: a,
            implicit: false,
            segment: 0,
        },
        Event::Captured {
            instance: a,
            segment: 0,
            occurrence: None,
        },
    ]);
    builder.on(&[]);
    builder.on(&[
        Event::LoopOpened {
            instance: b,
            implicit: true,
            segment: 1,
        },
        Event::Captured {
            instance: b,
            segment: 1,
            occurrence: None,
        },
        Event::Unmatched {
            segment: 2,
            within: Some(b),
        },
        Event::Empty { segment: 3 },
        Event::LoopClosed { instance: b },
        Event::LoopClosed { instance: a },
        Event::LoopClosed { instance: a },
        Event::Unmatched {
            segment: 4,
            within: None,
        },
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

#[test]
fn a_second_stream_hangs_from_the_root() {
    let spec = nested();
    let mut builder = LoopTree::builder();
    fold(&mut builder, &spec, b"AA~BB~");
    fold(&mut builder, &spec, b"AA~");
    let tree = builder.finish();
    let root = tree.node(tree.root());
    assert_eq!(root.children.len(), 2, "one A per stream");
    let first = tree.node(root.children[0]);
    let second = tree.node(root.children[1]);
    assert_eq!(first.children.len(), 1, "the first A keeps its B");
    assert!(second.children.is_empty());
    assert_eq!(first.segments, vec![0]);
    assert_eq!(second.segments, vec![0], "each A holds its own AA");
    assert_eq!(tree.node_count(), 4);
}

#[test]
fn an_unknown_instance_goes_to_the_root() {
    let spec = nested();
    let a_id = spec.loop_id("A").unwrap();
    let ghost = |index, parent| Instance {
        id: InstanceId::at(index),
        loop_id: a_id,
        ordinal_in_stream: 1,
        parent,
    };
    let mut builder = LoopTree::builder();
    builder.on(&[
        Event::Captured {
            instance: ghost(7, None),
            segment: 0,
            occurrence: None,
        },
        Event::Unmatched {
            segment: 1,
            within: Some(ghost(9, None)),
        },
        Event::LoopOpened {
            instance: ghost(2, Some(InstanceId::at(5))),
            implicit: false,
            segment: 2,
        },
    ]);
    let tree = builder.finish();
    let root = tree.node(tree.root());
    assert_eq!(root.segments, vec![0]);
    assert_eq!(root.unmatched, vec![1]);
    assert_eq!(root.children.len(), 1, "the orphan hangs from the root");
    assert_eq!(tree.node(root.children[0]).parent, Some(tree.root()));
}

#[test]
fn an_unmatched_segment_at_the_top_level_goes_to_the_root() {
    let (_, tree) = tree(b"ZZ~");
    assert_eq!(tree.node(tree.root()).unmatched, vec![0]);
}

#[test]
fn a_segment_goes_to_the_instance_it_names_not_the_last_one_opened() {
    let spec = nested();
    let a = Instance {
        id: InstanceId::at(0),
        loop_id: spec.loop_id("A").unwrap(),
        ordinal_in_stream: 1,
        parent: None,
    };
    let b = Instance {
        id: InstanceId::at(1),
        loop_id: spec.loop_id("B").unwrap(),
        ordinal_in_stream: 1,
        parent: Some(a.id),
    };
    let mut builder = LoopTree::builder();
    builder.on(&[
        Event::LoopOpened {
            instance: a,
            implicit: false,
            segment: 0,
        },
        Event::LoopOpened {
            instance: b,
            implicit: false,
            segment: 1,
        },
        Event::Captured {
            instance: a,
            segment: 2,
            occurrence: None,
        },
        Event::Unmatched {
            segment: 3,
            within: Some(a),
        },
    ]);
    let tree = builder.finish();
    let root = tree.node(tree.root());
    let a_node = tree.node(root.children[0]);
    assert_eq!(a_node.segments, vec![2]);
    assert_eq!(a_node.unmatched, vec![3]);
    let b_node = tree.node(a_node.children[0]);
    assert!(b_node.segments.is_empty() && b_node.unmatched.is_empty());
}

#[test]
fn an_empty_segment_belongs_to_the_innermost_open_node() {
    let (spec, tree) = tree(b"AA~BB~\n~AE~");
    let owner = tree.node(tree.owner(2));
    assert_eq!(owner.loop_id, spec.loop_id("B"));
}

#[test]
fn a_segment_no_event_named_is_the_roots() {
    let (_, tree) = tree(b"AA~");
    assert_eq!(tree.owner(10_000), tree.root());
}

#[test]
fn each_node_counts_its_loops_instances() {
    let (spec, tree) = tree(b"AA~AE~AA~AE~");
    let a = spec.loop_id("A").unwrap();
    let ordinals: Vec<_> = tree.nodes_of(a).map(|n| tree.node(n).ordinal).collect();
    assert_eq!(ordinals, vec![Some(1), Some(2)]);
    assert_eq!(tree.node(tree.root()).ordinal, None);
}
