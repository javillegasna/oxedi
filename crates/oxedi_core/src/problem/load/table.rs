impl fmt::Display for TableDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableDefError::EmptyName { what } => write!(f, "the {what} is empty"),
            TableDefError::DuplicateLoop { loop_name } => {
                write!(
                    f,
                    "loop {loop_name:?} is listed more than once in \"loops\""
                )
            }
            TableDefError::NoLoops => {
                write!(
                    f,
                    "\"loops\" is empty; a table anchors in at least one loop"
                )
            }
            TableDefError::UnknownLoop { name } => write!(f, "loop {name:?} does not exist"),
            TableDefError::ReservedName { name } => {
                write!(f, "the name {name:?} is taken by an automatic column")
            }
            TableDefError::RefTaken { name, table } => {
                write!(f, "\"ref\" {name:?} is already used by table {table:?}")
            }
            TableDefError::RepeatWithoutSegment => write!(
                f,
                "\"repeat\" requires \"segment\": only a segment's elements repeat"
            ),
            TableDefError::ZeroStep => write!(f, "\"repeat.step\" is 0"),
            TableDefError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based position; found 0")
            }
            TableDefError::OffsetBeyondStep { offset, step } => write!(
                f,
                "\"group_element\" {offset} is outside a group of {step} elements (offsets start at 0)"
            ),
            TableDefError::NestedAnchors { outer, inner } => write!(
                f,
                "anchor loops {outer:?} and {inner:?} nest; a table without \"segment\" anchors in loops that do not"
            ),
            TableDefError::SharedAnchor { loop_name, other } => write!(
                f,
                "loop {loop_name:?} already anchors table {other:?}; a loop anchors at most one table without \"segment\""
            ),
            TableDefError::UnrelatedAnchors {
                first,
                second,
                chains,
            } => write!(
                f,
                "anchor loops {first:?} and {second:?} sit under tables that are not one chain: {first:?} under {}, {second:?} under {}",
                render_chain(&chains.first),
                render_chain(&chains.second)
            ),
            TableDefError::UnrelatedLoop { loop_name, anchor } => write!(
                f,
                "loop {loop_name:?} is neither anchor loop {anchor:?}, a loop inside it nor a loop above it"
            ),
            TableDefError::UnknownOccurrence {
                loop_name,
                occurrence,
                known,
            } => write!(
                f,
                "loop {loop_name:?} has no occurrence {occurrence:?}; {}",
                Declared(known)
            ),
            TableDefError::OccurrenceAndSegment { key, written } => write!(
                f,
                "\"occurrence\" already names the segment and its qualifier; {key:?} ({written}) does not go with it"
            ),
            TableDefError::OccurrenceSegments {
                occurrence,
                segments,
            } => write!(
                f,
                "occurrence {occurrence:?} is segment {:?} in anchor loop {:?} but segment {:?} in anchor loop {:?}; name the loop to read with \"loop\"",
                segments.first_segment, segments.first_loop, segments.segment, segments.loop_name
            ),
            TableDefError::AfterAnchor {
                occurrence,
                positions,
            } => write!(
                f,
                "occurrence {occurrence:?} (position {}) of loop {:?} comes after anchor loop {:?} opens (position {}): its segments arrive after the rows are appended, so the column never fills",
                positions.pos, positions.loop_name, positions.anchor, positions.anchor_pos
            ),
            TableDefError::PickNeedsOccurrence { written } => write!(
                f,
                "\"pick\" ({written}) requires \"occurrence\": only a named occurrence has a known repeat"
            ),
            TableDefError::PickOnSingle {
                loop_name,
                occurrence,
                pick,
            } => write!(
                f,
                "occurrence {occurrence:?} of loop {loop_name:?} appears at most once, so \"pick\" ({pick}) has nothing to choose from"
            ),
            TableDefError::PickBeyondMax {
                nth,
                loop_name,
                occurrence,
                max,
            } => write!(
                f,
                "\"pick\" {nth} is past occurrence {occurrence:?} of loop {loop_name:?}, which repeats at most {max} times"
            ),
            TableDefError::SegmentNotHeld { segment, loop_name } => write!(
                f,
                "segment {segment:?} is neither the trigger nor a segment of loop {loop_name:?}, so it is never read there"
            ),
            TableDefError::AnchorSegmentOnly {
                key,
                anchor_segment,
                written,
            } => write!(
                f,
                "{key:?} ({written}) does not apply in a table anchored on segment {anchor_segment:?}: its columns read that segment"
            ),
            TableDefError::NeedsSegment => write!(
                f,
                "the column names no \"segment\" or \"occurrence\" to read"
            ),
            TableDefError::GroupWithoutRepeat => {
                write!(f, "\"group_element\" requires the table's \"repeat\"")
            }
            TableDefError::SourceCount { found } => write!(
                f,
                "a column takes exactly one of \"element\", \"group_element\" or \"segment_index\"; found {found}"
            ),
            TableDefError::ComponentOnIndex => {
                write!(f, "\"component\" does not apply to \"segment_index\"")
            }
            TableDefError::PhiNeedsText { kind, found } => {
                write!(
                    f,
                    "\"phi\": {kind:?} applies to a text column; this column is {found}"
                )
            }
            TableDefError::PhiNeedsDate { found } => {
                write!(
                    f,
                    "\"phi\": \"date\" applies to a date column; this column is {found}"
                )
            }
            TableDefError::PhiOnControlElement { segment, element } => {
                write!(
                    f,
                    "\"phi\" cannot flag {}: it is an envelope control number or count; findings about envelope controls quote it unmasked, so it cannot be flagged; remove \"phi\" from this column",
                    ElementRef::simple(segment.as_bytes(), *element)
                )
            }
            TableDefError::UnknownPhiSeed { name } => {
                write!(
                    f,
                    "\"phi_seed\" names column {name:?}, which the table does not have; name an existing column flagged \"phi\": \"id\", or set \"phi_seed\": null"
                )
            }
            TableDefError::PhiSeedNotId { name } => {
                write!(
                    f,
                    "\"phi_seed\" names column {name:?}, which is not flagged \"phi\": \"id\"; flag it \"id\", name another \"id\" column, or set \"phi_seed\": null"
                )
            }
        }
    }
}
