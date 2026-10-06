//! Extending the structure is done with data: a patch captures a segment the
//! built-in spec does not know, and a proprietary loop opens with no code.

mod common;

use oxedi_core::{Delimiters, Event, LoopTree, Spec, Tokenizer};

#[test]
fn a_patch_makes_the_bogus_trizetto_segment_captured() {
    let bytes = common::load_fixture("trizetto_sample.rmt");
    let builtin = Spec::builtin_835();
    let patched = builtin
        .merge_patch(r#"{"loops":{"1000A":{"occurrences":{"xx":{"segment":"XX","pos":9000}}}}}"#)
        .unwrap();
    let payer = patched.get(patched.loop_id("1000A").unwrap());
    let before = builtin.get(builtin.loop_id("1000A").unwrap());
    assert_eq!(
        payer.segments[..payer.segments.len() - 1],
        before.segments[..],
        "a merge patch adds an occurrence by name and keeps the built-in ones"
    );
    assert_eq!(payer.segments.last(), Some(&b"XX".to_vec()));
    let delims = Delimiters::from_isa(&bytes).unwrap();
    let unmatched = |spec: &Spec| {
        common::events_of(spec, &bytes, delims)
            .iter()
            .filter(|e| matches!(e, Event::Unmatched { .. }))
            .count()
    };
    assert_eq!(unmatched(&builtin), 1);
    assert_eq!(unmatched(&patched), 0);
    let tree = LoopTree::build(&patched, Tokenizer::new(&bytes).unwrap());
    let payer = tree
        .nodes_of(patched.loop_id("1000A").unwrap())
        .next()
        .unwrap();
    let ids: Vec<Vec<u8>> = {
        let segments: Vec<_> = Tokenizer::new(&bytes).unwrap().collect();
        tree.node(payer)
            .segments
            .iter()
            .map(|&i| segments[i].id.to_vec())
            .collect()
    };
    assert!(
        ids.contains(&b"XX".to_vec()),
        "XX now belongs to the payer loop"
    );
}

#[test]
fn a_patch_adding_n3_n4_to_loop_2100_captures_the_multi_claim_addresses() {
    let bytes = common::load_fixture("multi_claim_sample.txt");
    let builtin = Spec::builtin_835();
    let patched = builtin
        .merge_patch(
            r#"{"loops":{"2100":{"occurrences":{"n3":{"segment":"N3","pos":9000},"n4":{"segment":"N4","pos":9001}}}}}"#,
        )
        .unwrap();
    let delims = Delimiters::from_isa(&bytes).unwrap();
    let unmatched = |spec: &Spec| {
        common::events_of(spec, &bytes, delims)
            .iter()
            .filter(|e| matches!(e, Event::Unmatched { .. }))
            .count()
    };
    assert_eq!(unmatched(&builtin), 4, "N3 and N4 in each of two claims");
    assert_eq!(unmatched(&patched), 0);
}

#[test]
fn a_proprietary_loop_opens_and_captures_with_data_only() {
    let input = b"ST*835*1~LX*1~CLP*1*1*10*10**MC*1~ZZ1*A~ZZ2*B~SVC*HC:1*10*10~SE*7*1~";
    let delims = Delimiters::new(b'*', b':', b'~');
    let builtin = Spec::builtin_835();
    let patched = builtin
        .merge_patch(r#"{"loops":{"2100-ZZ":{"parent":"2100","trigger":{"segment":"ZZ1"},"occurrences":{"zz1":{"segment":"ZZ1","pos":0},"zz2":{"segment":"ZZ2","pos":1}}}}}"#)
        .unwrap();

    let tree = LoopTree::build(&builtin, Tokenizer::with_delimiters(input, delims));
    let claim = tree
        .nodes_of(builtin.loop_id("2100").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        tree.node(claim).unmatched,
        vec![3, 4],
        "ZZ1 and ZZ2 are unknown to the built-in"
    );

    let tree = LoopTree::build(&patched, Tokenizer::with_delimiters(input, delims));
    let zz = tree
        .nodes_of(patched.loop_id("2100-ZZ").unwrap())
        .next()
        .unwrap();
    assert_eq!(tree.node(zz).segments, vec![3, 4]);
    let claim = tree
        .nodes_of(patched.loop_id("2100").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        tree.node(claim).children.len(),
        2,
        "the proprietary loop and the 2110"
    );
    assert!(tree.node(claim).unmatched.is_empty());
}
