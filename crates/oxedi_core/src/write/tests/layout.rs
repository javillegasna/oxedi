//! The emitter's per-spec layout over the built-in specs.

use crate::spec::Spec;
use crate::write::layout::Layout;
use crate::write::*;

/// The layout both built-in specs make: the same loops, plan segments and
/// tables, so the same order, forced openers, group columns and
/// capacities.
const BUILT_IN_LAYOUT: &str = "Layout { order: [\
[Segment(0), Segment(1), Segment(2), Segment(3)], \
[Segment(0), Segment(1), Segment(2)], \
[Segment(0), Loop(LoopId(3))], \
[Segment(0), Segment(1), Segment(2), Segment(3), Segment(4), Segment(5), Segment(6), Segment(7), Loop(LoopId(4))], \
[Segment(0), Segment(1), Segment(2), Segment(3), Segment(4)], \
[Segment(0), Loop(LoopId(7))], \
[Segment(0), Loop(LoopId(5))], \
[Segment(0), Segment(1), Segment(2), Segment(3), Segment(4), Loop(LoopId(0)), Loop(LoopId(1)), Loop(LoopId(2)), Segment(5)]], \
forced: [[true, false, false, false], [true, false, false], [true], \
[true, false, false, false, false, false, false, false], [true, false, false, false, false], \
[false], [false], [false, false, false, false, false, false]], \
group_columns: [[], [], [11], [], [], [], [], []], \
capacity: [6, 1, 1, 6, 1] }";

#[test]
fn the_layout_of_the_built_ins_is_unchanged() {
    for spec in [Spec::builtin_835(), Spec::builtin_835_4010()] {
        let plan = WritePlan::new(&spec).unwrap();
        assert_eq!(format!("{:?}", Layout::new(&spec, &plan)), BUILT_IN_LAYOUT);
    }
}
