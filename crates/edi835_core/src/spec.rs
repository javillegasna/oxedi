//! Loop specifications: the data that tells the engine how a file is structured.
//!
//! A spec is plain JSON: a map of loops, each naming its parent, the segment
//! (and optional element conditions) that opens it, the segments it may hold
//! and an optional segment that closes it. An optional `segments` section
//! names and types the elements of each segment id, wherever the segment
//! appears. Loading compiles that into index-based definitions so the engine
//! never compares strings, and keeps the JSON value so patches can be applied
//! on top. Patches follow RFC 7386: objects merge key by key, while arrays and
//! scalars replace wholesale; see [`Spec::merge_patch`] for what that means
//! when extending a loop's segment list.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde_json::Value;

use crate::element::Element;
use crate::segment::Segment;

/// Index of a loop definition inside a [`Spec`]. A `LoopId` is only
/// meaningful for the `Spec` that produced it: using it with another spec,
/// including one produced by [`Spec::merge_patch`], indexes a different loop
/// or panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopId(usize);

impl LoopId {
    /// Position of the loop in [`Spec::loops`].
    pub fn index(self) -> usize {
        self.0
    }
}

/// What opens a loop: a segment id plus optional element conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Segment id, e.g. `CLP`.
    pub segment: Vec<u8>,
    /// `(1-based element position, required value)`, sorted by position.
    pub conditions: Vec<(usize, Vec<u8>)>,
}

impl Trigger {
    /// `true` when the segment has this id and every condition holds.
    pub fn matches(&self, segment: &Segment<'_>) -> bool {
        segment.id == self.segment.as_slice()
            && self.conditions.iter().all(|(position, value)| {
                segment.element(*position).and_then(Element::simple) == Some(value.as_slice())
            })
    }
}

/// One loop of the structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopDef {
    /// Name used in the JSON, e.g. `2100`.
    pub name: String,
    /// `None` for a top-level loop.
    pub parent: Option<LoopId>,
    /// What opens the loop. The trigger segment is captured by the loop.
    pub trigger: Trigger,
    /// Segments the loop holds after its trigger.
    pub segments: Vec<Vec<u8>>,
    /// Segment that is captured and then closes the loop, e.g. `SE`.
    pub end: Option<Vec<u8>>,
    /// How the end segment checks the loop it closes, for envelope loops.
    pub control: Option<Control>,
    /// Loops whose parent is this one, in spec order.
    pub children: Vec<LoopId>,
}

/// What a loop's end segment counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCount {
    /// Every segment from the trigger to the end segment, both included.
    Segments,
    /// The child loop instances opened by their own trigger.
    Children,
}

/// The control elements an envelope loop's trigger and end segment carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// 1-based position of the control number in the trigger, e.g. `ST02`.
    pub opener_element: usize,
    /// 1-based position of the same control number in the end segment, e.g. `SE02`.
    pub closer_element: usize,
    /// 1-based position of the count in the end segment, e.g. `SE01`.
    pub count_element: usize,
    /// What the count counts.
    pub count: ControlCount,
}

/// Why a loop's `control` was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// A position key holds 0.
    ZeroPosition {
        /// The key, e.g. `opener_element`.
        key: &'static str,
    },
    /// `count` is not `segments` or `children`.
    UnknownCount {
        /// The value as written.
        found: String,
    },
    /// The loop has no `end` segment to carry the count and control number.
    NoEnd,
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based element position; found 0")
            }
            ControlError::UnknownCount { found } => {
                write!(
                    f,
                    "\"count\" must be \"segments\" or \"children\"; found {found:?}"
                )
            }
            ControlError::NoEnd => write!(f, "the loop has no \"end\" segment to check"),
        }
    }
}

impl LoopDef {
    /// `true` when the loop holds `id` (as a listed segment or as its end).
    pub fn accepts(&self, id: &[u8]) -> bool {
        self.segments.iter().any(|segment| segment == id) || self.end.as_deref() == Some(id)
    }
}

/// The data type of an element, as the X12 standard names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    /// `AN`: a string.
    An,
    /// `ID`: a code from a list.
    Id,
    /// `N0` to `N9`: an integer with that many implied decimal places.
    N(u8),
    /// `R`: a decimal number with an explicit point, kept at `scale` places.
    R {
        /// Decimal places; 2 unless the spec says otherwise.
        scale: u8,
    },
    /// `DT`: a date, `CCYYMMDD` or `YYMMDD`.
    Dt,
    /// `TM`: a time, `HHMM` optionally followed by seconds and decimal seconds.
    Tm,
}

impl ElementType {
    /// Largest `scale` an `R` element may declare. An `R` value is held as an
    /// `i128` scaled by `10^scale` in a column of precision 38, so a scale of
    /// 18 still leaves 20 digits for the integer part.
    pub const MAX_SCALE: u8 = 18;

    /// Reads the type code of an element definition; `scale` is the
    /// definition's `scale` key, which only `R` accepts.
    fn parse(code: &str, scale: Option<u8>) -> Result<ElementType, ElementDefError> {
        let kind = match code.as_bytes() {
            b"AN" => ElementType::An,
            b"ID" => ElementType::Id,
            b"R" => ElementType::R {
                scale: scale.unwrap_or(2),
            },
            b"DT" => ElementType::Dt,
            b"TM" => ElementType::Tm,
            [b'N', digit @ b'0'..=b'9'] => ElementType::N(digit - b'0'),
            _ => {
                return Err(ElementDefError::UnknownType {
                    found: code.to_string(),
                });
            }
        };
        if scale.is_some() && !matches!(kind, ElementType::R { .. }) {
            return Err(ElementDefError::ScaleWithoutR {
                kind: code.to_string(),
            });
        }
        Ok(kind)
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementType::An => write!(f, "AN (string)"),
            ElementType::Id => write!(f, "ID (code)"),
            ElementType::N(places) => {
                write!(f, "N{places} (integer with {places} implied decimals)")
            }
            ElementType::R { scale } => write!(f, "R (decimal, scale {scale})"),
            ElementType::Dt => write!(f, "DT (date CCYYMMDD or YYMMDD)"),
            ElementType::Tm => write!(f, "TM (time HHMM, HHMMSS or HHMMSSD..)"),
        }
    }
}

/// One element of a segment, or one component of a composite element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementDef {
    /// Name used for the element downstream, e.g. `claim_submitter_id`.
    pub name: String,
    /// Data type.
    pub kind: ElementType,
    /// `true` when the element must be present and non-empty.
    pub required: bool,
    /// Minimum length, when the spec sets one.
    pub min: Option<usize>,
    /// Maximum length, when the spec sets one.
    pub max: Option<usize>,
    /// Components by 1-based position; empty for a simple element.
    pub composite: BTreeMap<usize, ElementDef>,
}

/// The elements of one segment id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentDef {
    /// Elements by 1-based position. Positions with no entry are opaque.
    pub elements: BTreeMap<usize, ElementDef>,
}

/// Why an element definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementDefError {
    /// The position key is not a 1-based integer in canonical form.
    NonCanonicalPosition,
    /// `name` is the empty string.
    EmptyName,
    /// Another element at the same level already has this name.
    DuplicateName {
        /// The repeated name.
        name: String,
        /// Position key of the element that used it first, as written.
        first: String,
    },
    /// `type` is not one of the known codes.
    UnknownType {
        /// The code as written.
        found: String,
    },
    /// `scale` was given for a type other than `R`.
    ScaleWithoutR {
        /// The type code as written.
        kind: String,
    },
    /// `min` is greater than `max`.
    MinAboveMax {
        /// The minimum as written.
        min: usize,
        /// The maximum as written.
        max: usize,
    },
    /// `composite` was given for a type other than `AN`.
    CompositeOnNonAn {
        /// The type code as written.
        kind: String,
    },
    /// A component declares a `composite` of its own.
    NestedComposite,
    /// `scale` is above [`ElementType::MAX_SCALE`].
    ScaleAboveMaximum {
        /// The scale as written.
        scale: u8,
    },
    /// `max` is 0, so no value could ever be valid.
    ZeroMax,
}

impl fmt::Display for ElementDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementDefError::NonCanonicalPosition => write!(
                f,
                "positions are 1-based integers written in canonical form"
            ),
            ElementDefError::EmptyName => write!(f, "\"name\" is empty"),
            ElementDefError::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by position {first:?}")
            }
            ElementDefError::UnknownType { found } => write!(
                f,
                "type {found:?} is not one of AN, ID, N0 to N9, R, DT, TM"
            ),
            ElementDefError::ScaleWithoutR { kind } => {
                write!(f, "\"scale\" applies only to type R; found type {kind:?}")
            }
            ElementDefError::MinAboveMax { min, max } => {
                write!(f, "\"min\" {min} is greater than \"max\" {max}")
            }
            ElementDefError::CompositeOnNonAn { kind } => {
                write!(f, "\"composite\" requires type AN; found type {kind:?}")
            }
            ElementDefError::NestedComposite => {
                write!(f, "a component cannot declare its own \"composite\"")
            }
            ElementDefError::ScaleAboveMaximum { scale } => write!(
                f,
                "\"scale\" {scale} is above the maximum of {}",
                ElementType::MAX_SCALE
            ),
            ElementDefError::ZeroMax => {
                write!(f, "\"max\" is 0; an element holds at least one character")
            }
        }
    }
}

/// Name of the automatic column that numbers a table's rows from 0, across
/// the whole stream.
pub const ROW_COLUMN: &str = "row";

/// Name of the automatic column that holds the index of a row's anchor
/// segment: the segment that opened the anchor loop instance, or the
/// anchored segment itself.
pub const SEGMENT_COLUMN: &str = "segment";

/// How a table's segment repeats a group of elements: one row per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    /// 1-based position of the first group's first element.
    pub from: usize,
    /// Elements per group.
    pub step: usize,
}

/// Where a column takes its value from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnSource {
    /// An element (or one of its components) of the first segment that
    /// matches `segment` and `conditions`, captured in the anchor loop
    /// instance, or in the first instance of `loop_id` inside it. In a table
    /// anchored on a segment, the anchor segment itself.
    Element {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
    /// The index of the first segment that matches, chosen as for `Element`.
    SegmentIndex {
        /// A loop inside the anchor loop to read from; `None` for the anchor loop.
        loop_id: Option<LoopId>,
        /// The segment id.
        segment: Vec<u8>,
        /// `(1-based element position, required value)`, sorted by position.
        conditions: Vec<(usize, Vec<u8>)>,
    },
    /// An element of the row's group, in a table whose segment repeats a
    /// group: position `from + k * step + offset` for group `k`.
    GroupElement {
        /// 0-based position inside the group.
        offset: usize,
        /// 1-based component position, to read one component of a composite.
        component: Option<usize>,
    },
}

/// One table of the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDef {
    /// Name used in the JSON, e.g. `claims`.
    pub name: String,
    /// Name of the column that refers to a row of this table from the
    /// tables below it, e.g. `claim`; the table's name unless the spec says.
    pub reference: String,
    /// The loops whose instances (or whose segments) give rows.
    pub loops: Vec<LoopId>,
    /// With a segment, one row per occurrence of it in an anchor loop
    /// instead of one row per loop instance.
    pub segment: Option<Vec<u8>>,
    /// With a repeat, one row per element group of the segment.
    pub repeat: Option<Repeat>,
    /// The declared columns by name, in name order.
    pub columns: Vec<(String, ColumnSource)>,
    /// The nearest table above this one, as an index into [`Spec::tables`].
    pub parent: Option<usize>,
    /// Every table above this one, outermost first; the last is `parent`.
    /// Each one gets an automatic column named after its `reference`.
    pub ancestors: Vec<usize>,
}

/// The tables above two anchor loops, outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorChains {
    /// The tables above the first anchor loop.
    pub first: Vec<String>,
    /// The tables above the second anchor loop.
    pub second: Vec<String>,
}

/// Why a table definition was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDefError {
    /// A table name, column name or `ref` is the empty string.
    EmptyName {
        /// Which one: `table name`, `column name` or `ref`.
        what: &'static str,
    },
    /// `loops` is empty.
    NoLoops,
    /// A loop name appears twice in `loops`.
    DuplicateLoop {
        /// The loop name as written.
        loop_name: String,
    },
    /// A loop name does not exist.
    UnknownLoop {
        /// The name as written.
        name: String,
    },
    /// A column name or `ref` is taken by an automatic column.
    ReservedName {
        /// The name as written.
        name: String,
    },
    /// Another table already uses this `ref`.
    RefTaken {
        /// The `ref` as written.
        name: String,
        /// The table that uses it first, in name order.
        table: String,
    },
    /// `repeat` was given without `segment`.
    RepeatWithoutSegment,
    /// `repeat.step` is 0.
    ZeroStep,
    /// A 1-based position holds 0.
    ZeroPosition {
        /// The key: `repeat.from`, `element` or `component`.
        key: &'static str,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The key as written.
        key: String,
    },
    /// `group_element` does not fit inside a group.
    OffsetBeyondStep {
        /// The offset as written.
        offset: usize,
        /// The group size.
        step: usize,
    },
    /// Two anchor loops of a table without `segment` nest.
    NestedAnchors {
        /// The enclosing loop.
        outer: String,
        /// The loop inside it.
        inner: String,
    },
    /// The loop already anchors another table without `segment`.
    SharedAnchor {
        /// The loop.
        loop_name: String,
        /// The table that anchors in it first, in name order.
        other: String,
    },
    /// Two anchor loops lead to tables above that are not one chain.
    UnrelatedAnchors {
        /// The anchor loop with the longest chain of tables above it.
        first: String,
        /// The anchor loop whose chain disagrees.
        second: String,
        /// The table names above each anchor loop.
        chains: Box<AnchorChains>,
    },
    /// A column's `loop` is not inside every anchor loop.
    NotADescendant {
        /// The column's loop.
        loop_name: String,
        /// The anchor loop it is not inside.
        anchor: String,
    },
    /// A segment is read from a loop that neither triggers on it nor holds it.
    SegmentNotHeld {
        /// The segment the column or table reads.
        segment: String,
        /// The loop that never holds it.
        loop_name: String,
    },
    /// A key that picks a segment is used in a table anchored on a segment.
    AnchorSegmentOnly {
        /// The key: `segment`, `loop` or `where`.
        key: &'static str,
        /// The segment the table is anchored on.
        anchor_segment: String,
        /// The key's value as the spec writes it, in JSON.
        written: String,
    },
    /// A column of a table without `segment` names no segment.
    NeedsSegment,
    /// `group_element` is used in a table without `repeat`.
    GroupWithoutRepeat,
    /// A column does not name exactly one value source.
    SourceCount {
        /// How many of `element`, `group_element` and `segment_index` it names.
        found: usize,
    },
    /// `component` is given with `segment_index`.
    ComponentOnIndex,
}

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
            TableDefError::BadPosition { key } => write!(
                f,
                "\"where\" position {key:?} is not a 1-based integer in canonical form"
            ),
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
            TableDefError::NotADescendant { loop_name, anchor } => {
                write!(f, "loop {loop_name:?} is not inside anchor loop {anchor:?}")
            }
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
            TableDefError::NeedsSegment => write!(f, "the column names no \"segment\" to read"),
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
        }
    }
}

/// Why a spec could not be loaded. Each variant names where in the spec the fault is.
#[derive(Debug)]
pub enum SpecError {
    /// The text is not valid JSON.
    Json(serde_json::Error),
    /// The JSON is valid but does not match the spec schema.
    Schema {
        /// The loop whose definition is malformed; `None` when the top level is.
        loop_name: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// Applying a patch produced an error.
    Patch {
        /// The error the patched spec (or the patch text) produced.
        source: Box<SpecError>,
    },
    /// A loop names a parent that does not exist.
    UnknownParent {
        /// The loop with the bad reference.
        loop_name: String,
        /// The missing parent.
        parent: String,
    },
    /// Following `parent` links never reaches a top-level loop.
    Cycle {
        /// The loops on the cycle in walk order, starting at the one that repeats.
        members: Vec<String>,
    },
    /// The `loops` map is empty.
    NoLoops {
        /// The spec's `name`.
        spec_name: String,
    },
    /// A segment id is the empty string.
    EmptySegmentId {
        /// The loop holding it; `None` for a key of the `segments` section.
        loop_name: Option<String>,
        /// Where it sits, as written: `trigger.segment`, `segments[<i>]`,
        /// `end`, or `segments.""` for the section.
        key: String,
    },
    /// A loop's `end` is the segment that opens it.
    EndIsTrigger {
        /// The loop.
        loop_name: String,
        /// The segment id both keys name.
        segment: String,
    },
    /// A `where` key is not a 1-based element position in canonical form.
    BadPosition {
        /// The loop with the bad key.
        loop_name: String,
        /// The key as written.
        position: String,
    },
    /// A value the schema requires to be an object is something else.
    NotAnObject {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.2100.trigger`); empty for the top level.
        path: String,
        /// What was found instead: `an array`, `a string`, `a number`,
        /// `a boolean` or `null`.
        found: &'static str,
    },
    /// A scalar the schema requires to be of one kind is of another.
    WrongType {
        /// Where the value sits, keys joined by `.` as written (e.g.
        /// `loops.env.control.opener_element`).
        path: String,
        /// What the schema requires, e.g. `a non-negative integer`.
        expected: &'static str,
        /// What was found instead: `a string`, `a number`, `a negative
        /// number`, `an array`, `null`, and so on.
        found: &'static str,
    },
    /// A segment definition does not match the schema.
    SegmentSchema {
        /// The segment id as written.
        segment: String,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// An element definition is invalid.
    BadElementDef {
        /// The segment id as written.
        segment: String,
        /// The element's key as written; a component is `<element>.composite.<component>`.
        position: String,
        /// What is wrong with it.
        reason: ElementDefError,
    },
    /// A table or column definition does not match the schema.
    TableSchema {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What serde rejected.
        source: serde_json::Error,
    },
    /// A table definition is invalid.
    BadTable {
        /// The table name as written.
        table: String,
        /// The column name as written, when the fault is inside a column.
        column: Option<String>,
        /// What is wrong with it.
        reason: TableDefError,
    },
    /// A loop's `control` is invalid.
    BadControl {
        /// The loop.
        loop_name: String,
        /// What is wrong with it.
        reason: ControlError,
    },
    /// Two loops with the same parent have identical triggers.
    AmbiguousTrigger {
        /// First loop, in spec order.
        first: String,
        /// Second loop, in spec order.
        second: String,
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// The shared trigger segment id.
        segment: String,
        /// The shared trigger conditions, sorted by position.
        conditions: Vec<(usize, String)>,
    },
    /// Two loops with the same parent trigger on the same segment id, no
    /// position tested by both requires different values, and neither set of
    /// conditions contains the other: one segment can satisfy both and
    /// neither trigger is more specific.
    OverlappingTriggers {
        /// Their common parent; `None` for top-level loops.
        parent: Option<String>,
        /// First loop, in spec order.
        a: String,
        /// Second loop, in spec order.
        b: String,
        /// The first loop's trigger, e.g. `"N1" where {1: "PR"}`.
        conditions_a: String,
        /// The second loop's trigger, written the same way.
        conditions_b: String,
    },
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecError::Json(e) => write!(f, "invalid spec JSON: {e}"),
            SpecError::Schema {
                loop_name: Some(loop_name),
                source,
            } => write!(f, "loop {loop_name:?} does not match the schema: {source}"),
            SpecError::Schema {
                loop_name: None,
                source,
            } => write!(f, "spec does not match the schema: {source}"),
            SpecError::NotAnObject { path, found } if path.is_empty() => {
                write!(f, "the spec must be a JSON object; found {found}")
            }
            SpecError::NotAnObject { path, found } => write!(
                f,
                "spec: the value at {path} must be a JSON object; found {found}"
            ),
            SpecError::WrongType {
                path,
                expected,
                found,
            } => write!(
                f,
                "spec: the value at {path} must be {expected}; found {found}"
            ),
            SpecError::Patch { source } => write!(f, "applying patch: {source}"),
            SpecError::UnknownParent { loop_name, parent } => {
                write!(f, "loop {loop_name:?} names unknown parent {parent:?}")
            }
            SpecError::Cycle { members } => {
                write!(f, "loops form a parent cycle: ")?;
                for member in members {
                    write!(f, "{member} -> ")?;
                }
                match members.first() {
                    Some(first) => write!(f, "{first}"),
                    None => Ok(()),
                }
            }
            SpecError::NoLoops { spec_name } => {
                write!(f, "spec {spec_name:?} declares no loops")
            }
            SpecError::EmptySegmentId {
                loop_name: Some(loop_name),
                key,
            } => write!(f, "loop {loop_name:?} has an empty segment id at {key}"),
            SpecError::EmptySegmentId {
                loop_name: None,
                key,
            } => write!(f, "the spec has an empty segment id at {key}"),
            SpecError::EndIsTrigger { loop_name, segment } => write!(
                f,
                "loop {loop_name:?} has \"end\" {segment:?}, the same segment as its \
                 \"trigger\": the loop would close on the segment that opens it"
            ),
            SpecError::BadPosition {
                loop_name,
                position,
            } => write!(
                f,
                "loop {loop_name:?} has an invalid \"where\" position {position:?}: \
                 positions are 1-based integers written in canonical form"
            ),
            SpecError::SegmentSchema { segment, source } => {
                write!(f, "segment {segment:?} does not match the schema: {source}")
            }
            SpecError::BadElementDef {
                segment,
                position,
                reason,
            } => write!(f, "segment {segment:?} element {position:?}: {reason}"),
            SpecError::TableSchema {
                table,
                column: None,
                source,
            } => write!(f, "table {table:?} does not match the schema: {source}"),
            SpecError::TableSchema {
                table,
                column: Some(column),
                source,
            } => write!(
                f,
                "table {table:?} column {column:?} does not match the schema: {source}"
            ),
            SpecError::BadTable {
                table,
                column: None,
                reason,
            } => write!(f, "table {table:?}: {reason}"),
            SpecError::BadTable {
                table,
                column: Some(column),
                reason,
            } => write!(f, "table {table:?} column {column:?}: {reason}"),
            SpecError::BadControl { loop_name, reason } => {
                write!(f, "loop {loop_name:?} has an invalid \"control\": {reason}")
            }
            SpecError::AmbiguousTrigger {
                first,
                second,
                parent,
                segment,
                conditions,
            } => {
                write!(f, "loops {first:?} and {second:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(f, " share the identical trigger {segment:?}")?;
                if !conditions.is_empty() {
                    write!(f, " where {{")?;
                    for (i, (position, value)) in conditions.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{position}: {value:?}")?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            SpecError::OverlappingTriggers {
                parent,
                a,
                b,
                conditions_a,
                conditions_b,
            } => {
                write!(f, "loops {a:?} and {b:?} under ")?;
                match parent {
                    Some(parent) => write!(f, "{parent:?}")?,
                    None => write!(f, "the root")?,
                }
                write!(
                    f,
                    " can open on the same segment: {a:?} on {conditions_a}, {b:?} on \
                     {conditions_b}, no position they both test requires different values, \
                     and neither trigger is more specific than the other"
                )
            }
        }
    }
}

impl std::error::Error for SpecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SpecError::Json(e)
            | SpecError::Schema { source: e, .. }
            | SpecError::SegmentSchema { source: e, .. }
            | SpecError::TableSchema { source: e, .. } => Some(e),
            SpecError::Patch { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

// Loops are kept as raw values so each one is deserialized on its own and a
// schema error can name the loop it came from.
#[derive(Debug, Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = "a spec object with \"name\" and \"loops\""
)]
struct RawSpec {
    name: String,
    loops: BTreeMap<String, Value>,
    #[serde(default)]
    segments: BTreeMap<String, Value>,
    #[serde(default)]
    tables: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a loop object")]
struct RawLoop {
    parent: Option<String>,
    trigger: RawTrigger,
    #[serde(default)]
    segments: Vec<String>,
    end: Option<String>,
    control: Option<RawControl>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a control object")]
struct RawControl {
    opener_element: usize,
    closer_element: usize,
    count_element: usize,
    count: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a trigger object")]
struct RawTrigger {
    segment: String,
    #[serde(default, rename = "where")]
    conditions: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a segment object")]
struct RawSegment {
    #[serde(default)]
    elements: BTreeMap<String, RawElement>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "an element object")]
struct RawElement {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    required: bool,
    min: Option<usize>,
    max: Option<usize>,
    scale: Option<u8>,
    #[serde(default)]
    composite: BTreeMap<String, RawElement>,
}

// Columns are kept as raw values so a schema error can name the column.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a table object")]
struct RawTable {
    loops: Vec<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    segment: Option<String>,
    repeat: Option<RawRepeat>,
    #[serde(default)]
    columns: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a repeat object")]
struct RawRepeat {
    from: usize,
    step: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, expecting = "a column object")]
struct RawColumn {
    #[serde(rename = "loop")]
    loop_name: Option<String>,
    segment: Option<String>,
    #[serde(default, rename = "where")]
    conditions: BTreeMap<String, String>,
    element: Option<usize>,
    component: Option<usize>,
    group_element: Option<usize>,
    #[serde(default)]
    segment_index: bool,
}

/// A loaded, validated loop structure.
#[derive(Debug, Clone)]
pub struct Spec {
    name: String,
    loops: Vec<LoopDef>,
    roots: Vec<LoopId>,
    segments: BTreeMap<Vec<u8>, SegmentDef>,
    tables: Vec<TableDef>,
    source: Value,
}

impl Spec {
    /// The built-in 835 structure as shipped, in the same JSON a user would write.
    pub const BUILTIN_835_JSON: &'static str = include_str!("../specs/835.json");

    /// The built-in 835 structure.
    pub fn builtin_835() -> Spec {
        Spec::from_json(Self::BUILTIN_835_JSON)
            .expect("the built-in 835 spec is valid; spec::tests::builtin_835_loads checks it")
    }

    /// Loads and validates a spec from JSON text.
    pub fn from_json(json: &str) -> Result<Spec, SpecError> {
        let source: Value = serde_json::from_str(json).map_err(SpecError::Json)?;
        Spec::from_value(source)
    }

    /// Applies a JSON Merge Patch (RFC 7386) to this spec's JSON and loads the
    /// result: objects merge recursively, arrays and scalars are replaced,
    /// `null` deletes. The result is validated like any spec; every failure,
    /// including unparsable patch text, is wrapped in [`SpecError::Patch`].
    ///
    /// Because `segments` is an array, a patch that touches it replaces the whole
    /// list rather than appending. To add a segment to a loop, the patch must
    /// list the loop's complete segment list, including the built-in entries that
    /// would otherwise be lost. Patches that only change `trigger`, `parent`, or
    /// `end` leave `segments` untouched, since objects merge key by key. The
    /// current segment list is visible through [`Self::to_json()`] or [`Self::get()`].
    ///
    /// # Example
    ///
    /// ```
    /// # use edi835_core::Spec;
    /// let spec = Spec::builtin_835();
    /// let patched = spec.merge_patch(
    ///     r#"{"loops":{"1000A":{"segments":["N3","N4","REF","PER","XX"]}}}"#
    /// ).unwrap();
    /// let loop_1000a = patched.get(patched.loop_id("1000A").unwrap());
    /// assert_eq!(loop_1000a.segments.len(), 5);
    /// assert!(loop_1000a.segments.contains(&b"XX".to_vec()));
    /// ```
    pub fn merge_patch(&self, patch_json: &str) -> Result<Spec, SpecError> {
        let patched = serde_json::from_str::<Value>(patch_json)
            .map_err(SpecError::Json)
            .and_then(|patch| {
                let mut source = self.source.clone();
                merge_patch(&mut source, &patch);
                Spec::from_value(source)
            });
        patched.map_err(|e| SpecError::Patch {
            source: Box::new(e),
        })
    }

    /// The spec as JSON text, including any patches applied to it.
    pub fn to_json(&self) -> String {
        // A `Value` always serializes; a failure here would be a bug in serde_json.
        serde_json::to_string_pretty(&self.source).unwrap_or_default()
    }

    /// The spec's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every loop, in spec order. Spec order is alphabetical by loop name
    /// (the JSON map is read into a `BTreeMap`), and it breaks ties between
    /// equally specific triggers.
    pub fn loops(&self) -> &[LoopDef] {
        &self.loops
    }

    /// The loop behind an id.
    pub fn get(&self, id: LoopId) -> &LoopDef {
        &self.loops[id.0]
    }

    /// The id of a loop by name.
    pub fn loop_id(&self, name: &str) -> Option<LoopId> {
        self.loops
            .iter()
            .position(|def| def.name == name)
            .map(LoopId)
    }

    /// The name of a loop by id.
    pub fn loop_name(&self, id: LoopId) -> &str {
        &self.loops[id.0].name
    }

    /// Top-level loops (no parent), in spec order.
    pub fn roots(&self) -> &[LoopId] {
        &self.roots
    }

    /// The element definitions of a segment id, if the spec has any.
    pub fn segment(&self, id: &[u8]) -> Option<&SegmentDef> {
        self.segments.get(id)
    }

    /// Every defined segment id with its definition, ordered by id.
    pub fn segments(&self) -> impl Iterator<Item = (&[u8], &SegmentDef)> {
        self.segments.iter().map(|(id, def)| (id.as_slice(), def))
    }

    /// The definition of an element, or of one of its components; `None`
    /// when the spec does not define it.
    pub fn element_def(
        &self,
        segment: &[u8],
        element: usize,
        component: Option<usize>,
    ) -> Option<&ElementDef> {
        let def = self.segments.get(segment)?.elements.get(&element)?;
        match component {
            None => Some(def),
            Some(component) => def.composite.get(&component),
        }
    }

    /// Every table, in name order.
    pub fn tables(&self) -> &[TableDef] {
        &self.tables
    }

    /// A table by name.
    pub fn table(&self, name: &str) -> Option<&TableDef> {
        self.tables.iter().find(|table| table.name == name)
    }

    /// Children of a loop, or the top-level loops for `None`.
    pub fn children(&self, parent: Option<LoopId>) -> &[LoopId] {
        match parent {
            Some(id) => &self.loops[id.0].children,
            None => &self.roots,
        }
    }

    /// Ancestors of a loop, root-most first, excluding the loop itself.
    pub fn ancestors(&self, id: LoopId) -> Vec<LoopId> {
        let mut chain = Vec::new();
        let mut current = self.loops[id.0].parent;
        while let Some(parent) = current {
            chain.push(parent);
            current = self.loops[parent.0].parent;
        }
        chain.reverse();
        chain
    }

    /// The child of `parent` that `segment` triggers, preferring the trigger
    /// with the most conditions; ties go to spec order.
    pub fn matching_child(&self, parent: Option<LoopId>, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match(self.children(parent).iter().copied(), segment)
    }

    /// Any loop that `segment` triggers, wherever it sits in the structure.
    pub fn matching_any(&self, segment: &Segment<'_>) -> Option<LoopId> {
        self.best_match((0..self.loops.len()).map(LoopId), segment)
    }

    fn best_match(
        &self,
        candidates: impl Iterator<Item = LoopId>,
        segment: &Segment<'_>,
    ) -> Option<LoopId> {
        candidates
            .filter(|&id| self.loops[id.0].trigger.matches(segment))
            .min_by_key(|&id| Reverse(self.loops[id.0].trigger.conditions.len()))
    }

    pub(crate) fn from_value(source: Value) -> Result<Spec, SpecError> {
        check_shape(&source)?;
        let raw: RawSpec =
            serde_json::from_value(source.clone()).map_err(|e| SpecError::Schema {
                loop_name: None,
                source: e,
            })?;
        if raw.loops.is_empty() {
            return Err(SpecError::NoLoops {
                spec_name: raw.name,
            });
        }
        let mut defs = Vec::with_capacity(raw.loops.len());
        for (name, value) in &raw.loops {
            let def: RawLoop =
                serde_json::from_value(value.clone()).map_err(|e| SpecError::Schema {
                    loop_name: Some(name.clone()),
                    source: e,
                })?;
            defs.push((name.clone(), def));
        }
        let names: Vec<&str> = raw.loops.keys().map(String::as_str).collect();
        let id_of = |name: &str| names.iter().position(|&n| n == name).map(LoopId);

        let mut loops = Vec::with_capacity(defs.len());
        for (name, def) in &defs {
            let empty_at = |key: String| SpecError::EmptySegmentId {
                loop_name: Some(name.clone()),
                key,
            };
            if def.trigger.segment.is_empty() {
                return Err(empty_at("trigger.segment".into()));
            }
            if let Some(i) = def.segments.iter().position(String::is_empty) {
                return Err(empty_at(format!("segments[{i}]")));
            }
            if def.end.as_deref() == Some("") {
                return Err(empty_at("end".into()));
            }
            if def.end.as_deref() == Some(def.trigger.segment.as_str()) {
                return Err(SpecError::EndIsTrigger {
                    loop_name: name.clone(),
                    segment: def.trigger.segment.clone(),
                });
            }
            let parent = match &def.parent {
                None => None,
                Some(parent) => Some(id_of(parent).ok_or_else(|| SpecError::UnknownParent {
                    loop_name: name.clone(),
                    parent: parent.clone(),
                })?),
            };
            let mut conditions = Vec::with_capacity(def.trigger.conditions.len());
            for (position, value) in &def.trigger.conditions {
                let parsed = parse_position(position).ok_or_else(|| SpecError::BadPosition {
                    loop_name: name.clone(),
                    position: position.clone(),
                })?;
                conditions.push((parsed, value.as_bytes().to_vec()));
            }
            conditions.sort();
            let control = match &def.control {
                None => None,
                Some(raw) => Some(compile_control(raw, def.end.is_some()).map_err(|reason| {
                    SpecError::BadControl {
                        loop_name: name.clone(),
                        reason,
                    }
                })?),
            };
            loops.push(LoopDef {
                name: name.clone(),
                parent,
                trigger: Trigger {
                    segment: def.trigger.segment.as_bytes().to_vec(),
                    conditions,
                },
                segments: def.segments.iter().map(|s| s.as_bytes().to_vec()).collect(),
                end: def.end.as_ref().map(|s| s.as_bytes().to_vec()),
                control,
                children: Vec::new(),
            });
        }

        check_cycles(&loops)?;

        let parents: Vec<Option<LoopId>> = loops.iter().map(|def| def.parent).collect();
        let mut roots = Vec::new();
        for (index, parent) in parents.into_iter().enumerate() {
            match parent {
                Some(parent) => loops[parent.0].children.push(LoopId(index)),
                None => roots.push(LoopId(index)),
            }
        }

        let mut segments = BTreeMap::new();
        for (id, value) in &raw.segments {
            if id.is_empty() {
                return Err(SpecError::EmptySegmentId {
                    loop_name: None,
                    key: "segments.\"\"".into(),
                });
            }
            let def: RawSegment =
                serde_json::from_value(value.clone()).map_err(|e| SpecError::SegmentSchema {
                    segment: id.clone(),
                    source: e,
                })?;
            let elements = compile_elements(id, &def.elements, None)?;
            segments.insert(id.as_bytes().to_vec(), SegmentDef { elements });
        }

        let mut spec = Spec {
            name: raw.name,
            loops,
            roots,
            segments,
            tables: Vec::new(),
            source,
        };
        spec.check_ambiguity()?;
        spec.tables = compile_tables(&spec, &raw.tables)?;
        Ok(spec)
    }

    fn check_ambiguity(&self) -> Result<(), SpecError> {
        let groups = std::iter::once(self.roots.as_slice())
            .chain(self.loops.iter().map(|def| def.children.as_slice()));
        for siblings in groups {
            for (i, &first) in siblings.iter().enumerate() {
                for &second in &siblings[i + 1..] {
                    let trigger = &self.loops[first.0].trigger;
                    let other = &self.loops[second.0].trigger;
                    if trigger.segment != other.segment {
                        continue;
                    }
                    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                    let parent = self.loops[first.0]
                        .parent
                        .map(|parent| self.loops[parent.0].name.clone());
                    if trigger == other {
                        return Err(SpecError::AmbiguousTrigger {
                            first: self.loops[first.0].name.clone(),
                            second: self.loops[second.0].name.clone(),
                            parent,
                            segment: text(&trigger.segment),
                            conditions: trigger
                                .conditions
                                .iter()
                                .map(|(position, value)| (*position, text(value)))
                                .collect(),
                        });
                    }
                    // Siblings are told apart when a shared position requires
                    // different values, or when one trigger's conditions
                    // contain the other's: the engine then prefers the one
                    // with more conditions and the other is the catch-all.
                    let excluded = trigger.conditions.iter().any(|(position, value)| {
                        other
                            .conditions
                            .iter()
                            .any(|(p, v)| p == position && v != value)
                    });
                    let contains = |big: &Trigger, small: &Trigger| {
                        small
                            .conditions
                            .iter()
                            .all(|condition| big.conditions.contains(condition))
                    };
                    let nested = contains(trigger, other) || contains(other, trigger);
                    if !excluded && !nested {
                        return Err(SpecError::OverlappingTriggers {
                            parent,
                            a: self.loops[first.0].name.clone(),
                            b: self.loops[second.0].name.clone(),
                            conditions_a: render_trigger(trigger),
                            conditions_b: render_trigger(other),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Table names outermost first, joined by `/`; `no table` when there are none.
fn render_chain(chain: &[String]) -> String {
    if chain.is_empty() {
        return "no table".to_string();
    }
    let names: Vec<Cow<'_, str>> = chain.iter().map(|name| render_key(name)).collect();
    names.join("/")
}

/// Compiles the `tables` section against the loops of `spec`, then links
/// every table to the tables above it.
fn compile_tables(spec: &Spec, raw: &BTreeMap<String, Value>) -> Result<Vec<TableDef>, SpecError> {
    let mut tables = Vec::with_capacity(raw.len());
    for (name, value) in raw {
        let def: RawTable =
            serde_json::from_value(value.clone()).map_err(|e| SpecError::TableSchema {
                table: name.clone(),
                column: None,
                source: e,
            })?;
        tables.push(compile_table(spec, name, &def)?);
    }
    link_tables(spec, &mut tables)?;
    Ok(tables)
}

fn compile_table(spec: &Spec, name: &str, def: &RawTable) -> Result<TableDef, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: name.to_string(),
        column: None,
        reason,
    };
    if name.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "table name" }));
    }
    if def.loops.is_empty() {
        return Err(fail(TableDefError::NoLoops));
    }
    let mut loops = Vec::with_capacity(def.loops.len());
    for loop_name in &def.loops {
        let id = spec.loop_id(loop_name).ok_or_else(|| {
            fail(TableDefError::UnknownLoop {
                name: loop_name.clone(),
            })
        })?;
        if loops.contains(&id) {
            return Err(fail(TableDefError::DuplicateLoop {
                loop_name: loop_name.clone(),
            }));
        }
        loops.push(id);
    }
    let reference = def.reference.clone().unwrap_or_else(|| name.to_string());
    if reference.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "ref" }));
    }
    if reference == ROW_COLUMN || reference == SEGMENT_COLUMN {
        return Err(fail(TableDefError::ReservedName { name: reference }));
    }
    let segment = match def.segment.as_deref() {
        None => None,
        Some("") => {
            return Err(SpecError::EmptySegmentId {
                loop_name: None,
                key: format!("tables.{}.segment", render_key(name)),
            });
        }
        Some(id) => Some(id.as_bytes().to_vec()),
    };
    let repeat = match &def.repeat {
        None => None,
        Some(_) if segment.is_none() => return Err(fail(TableDefError::RepeatWithoutSegment)),
        Some(raw) if raw.from == 0 => {
            return Err(fail(TableDefError::ZeroPosition { key: "repeat.from" }));
        }
        Some(raw) if raw.step == 0 => return Err(fail(TableDefError::ZeroStep)),
        Some(raw) => Some(Repeat {
            from: raw.from,
            step: raw.step,
        }),
    };
    if let Some(anchor) = &segment {
        check_held(spec, &loops, anchor).map_err(fail)?;
    }
    if segment.is_none() {
        for &a in &loops {
            for &b in &loops {
                if spec.ancestors(b).contains(&a) {
                    return Err(fail(TableDefError::NestedAnchors {
                        outer: spec.loop_name(a).to_string(),
                        inner: spec.loop_name(b).to_string(),
                    }));
                }
            }
        }
    }
    let mut columns = Vec::with_capacity(def.columns.len());
    for (column, value) in &def.columns {
        let raw: RawColumn =
            serde_json::from_value(value.clone()).map_err(|e| SpecError::TableSchema {
                table: name.to_string(),
                column: Some(column.clone()),
                source: e,
            })?;
        let source = compile_column(spec, name, column, &raw, &loops, segment.as_deref(), repeat)?;
        columns.push((column.clone(), source));
    }
    Ok(TableDef {
        name: name.to_string(),
        reference,
        loops,
        segment,
        repeat,
        columns,
        parent: None,
        ancestors: Vec::new(),
    })
}

fn compile_column(
    spec: &Spec,
    table: &str,
    column: &str,
    raw: &RawColumn,
    anchors: &[LoopId],
    anchor_segment: Option<&[u8]>,
    repeat: Option<Repeat>,
) -> Result<ColumnSource, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: table.to_string(),
        column: Some(column.to_string()),
        reason,
    };
    if column.is_empty() {
        return Err(fail(TableDefError::EmptyName {
            what: "column name",
        }));
    }
    if column == ROW_COLUMN || column == SEGMENT_COLUMN {
        return Err(fail(TableDefError::ReservedName {
            name: column.to_string(),
        }));
    }
    let found = usize::from(raw.element.is_some())
        + usize::from(raw.group_element.is_some())
        + usize::from(raw.segment_index);
    if found != 1 {
        return Err(fail(TableDefError::SourceCount { found }));
    }
    if raw.segment_index && raw.component.is_some() {
        return Err(fail(TableDefError::ComponentOnIndex));
    }
    if raw.element == Some(0) {
        return Err(fail(TableDefError::ZeroPosition { key: "element" }));
    }
    if raw.component == Some(0) {
        return Err(fail(TableDefError::ZeroPosition { key: "component" }));
    }
    if anchor_segment.is_some() {
        let keys = [
            ("segment", raw.segment.is_some()),
            ("loop", raw.loop_name.is_some()),
            ("where", !raw.conditions.is_empty()),
        ];
        if let Some(&(key, _)) = keys.iter().find(|(_, present)| *present) {
            let value = match key {
                "segment" => serde_json::to_string(&raw.segment),
                "loop" => serde_json::to_string(&raw.loop_name),
                _ => serde_json::to_string(&raw.conditions),
            };
            return Err(fail(TableDefError::AnchorSegmentOnly {
                key,
                anchor_segment: String::from_utf8_lossy(anchor_segment.unwrap_or_default())
                    .into_owned(),
                written: value.unwrap_or_default(),
            }));
        }
    }
    if let Some(offset) = raw.group_element {
        let Some(repeat) = repeat else {
            return Err(fail(TableDefError::GroupWithoutRepeat));
        };
        if offset >= repeat.step {
            return Err(fail(TableDefError::OffsetBeyondStep {
                offset,
                step: repeat.step,
            }));
        }
        return Ok(ColumnSource::GroupElement {
            offset,
            component: raw.component,
        });
    }
    let (loop_id, segment, conditions) = match anchor_segment {
        Some(anchor) => (None, anchor.to_vec(), Vec::new()),
        None => {
            let segment = match raw.segment.as_deref() {
                None => return Err(fail(TableDefError::NeedsSegment)),
                Some("") => {
                    return Err(SpecError::EmptySegmentId {
                        loop_name: None,
                        key: format!(
                            "tables.{}.columns.{}.segment",
                            render_key(table),
                            render_key(column)
                        ),
                    });
                }
                Some(id) => id.as_bytes().to_vec(),
            };
            let loop_id = match &raw.loop_name {
                None => None,
                Some(name) => {
                    let id = spec
                        .loop_id(name)
                        .ok_or_else(|| fail(TableDefError::UnknownLoop { name: name.clone() }))?;
                    if let Some(&anchor) = anchors
                        .iter()
                        .find(|&&anchor| !spec.ancestors(id).contains(&anchor))
                    {
                        return Err(fail(TableDefError::NotADescendant {
                            loop_name: name.clone(),
                            anchor: spec.loop_name(anchor).to_string(),
                        }));
                    }
                    Some(id)
                }
            };
            let mut conditions = Vec::with_capacity(raw.conditions.len());
            for (key, value) in &raw.conditions {
                let position = parse_position(key)
                    .ok_or_else(|| fail(TableDefError::BadPosition { key: key.clone() }))?;
                conditions.push((position, value.as_bytes().to_vec()));
            }
            conditions.sort();
            if raw.element.is_some() {
                let readers = loop_id.map_or_else(|| anchors.to_vec(), |id| vec![id]);
                check_held(spec, &readers, &segment).map_err(fail)?;
            }
            (loop_id, segment, conditions)
        }
    };
    Ok(match raw.element {
        Some(element) => ColumnSource::Element {
            loop_id,
            segment,
            conditions,
            element,
            component: raw.component,
        },
        None => ColumnSource::SegmentIndex {
            loop_id,
            segment,
            conditions,
        },
    })
}

/// Fails with the first loop of `readers` that neither triggers on `segment`
/// nor holds it.
fn check_held(spec: &Spec, readers: &[LoopId], segment: &[u8]) -> Result<(), TableDefError> {
    for &id in readers {
        let def = spec.get(id);
        if def.trigger.segment != segment && !def.accepts(segment) {
            return Err(TableDefError::SegmentNotHeld {
                segment: String::from_utf8_lossy(segment).into_owned(),
                loop_name: def.name.clone(),
            });
        }
    }
    Ok(())
}

/// Rejects a loop anchoring two tables without `segment` and a `ref` used
/// twice, then gives each table its chain of tables above: the tables
/// anchored on the loops above each anchor (from the anchor loop itself for
/// a table anchored on a segment, whose rows live inside that loop). Chains
/// run outermost first, and every anchor's chain must begin the longest one.
fn link_tables(spec: &Spec, tables: &mut [TableDef]) -> Result<(), SpecError> {
    let bad = |table: &TableDef, column: Option<&str>, reason| SpecError::BadTable {
        table: table.name.clone(),
        column: column.map(str::to_string),
        reason,
    };
    let mut anchored: Vec<Option<usize>> = vec![None; spec.loops().len()];
    for (index, table) in tables.iter().enumerate() {
        if table.segment.is_some() {
            continue;
        }
        for &id in &table.loops {
            if let Some(other) = anchored[id.index()] {
                return Err(bad(
                    table,
                    None,
                    TableDefError::SharedAnchor {
                        loop_name: spec.loop_name(id).to_string(),
                        other: tables[other].name.clone(),
                    },
                ));
            }
            anchored[id.index()] = Some(index);
        }
    }
    for (index, table) in tables.iter().enumerate() {
        if let Some(first) = tables[..index]
            .iter()
            .find(|other| other.reference == table.reference)
        {
            return Err(bad(
                table,
                None,
                TableDefError::RefTaken {
                    name: table.reference.clone(),
                    table: first.name.clone(),
                },
            ));
        }
    }
    for index in 0..tables.len() {
        let table = &tables[index];
        let mut longest: Option<(LoopId, Vec<usize>)> = None;
        let mut chains = Vec::with_capacity(table.loops.len());
        for &anchor in &table.loops {
            let mut chain = Vec::new();
            let mut current = if table.segment.is_some() {
                Some(anchor)
            } else {
                spec.get(anchor).parent
            };
            while let Some(id) = current {
                if let Some(owner) = anchored[id.index()] {
                    chain.push(owner);
                }
                current = spec.get(id).parent;
            }
            chain.reverse();
            if longest
                .as_ref()
                .is_none_or(|(_, best)| chain.len() > best.len())
            {
                longest = Some((anchor, chain.clone()));
            }
            chains.push((anchor, chain));
        }
        let Some((first, ancestors)) = longest else {
            continue;
        };
        for (anchor, chain) in &chains {
            if !ancestors.starts_with(chain) {
                return Err(bad(
                    table,
                    None,
                    TableDefError::UnrelatedAnchors {
                        first: spec.loop_name(first).to_string(),
                        second: spec.loop_name(*anchor).to_string(),
                        chains: Box::new(AnchorChains {
                            first: ancestors
                                .iter()
                                .map(|&above| tables[above].name.clone())
                                .collect(),
                            second: chain
                                .iter()
                                .map(|&above| tables[above].name.clone())
                                .collect(),
                        }),
                    },
                ));
            }
        }
        for (column, _) in &table.columns {
            if ancestors
                .iter()
                .any(|&above| tables[above].reference == *column)
            {
                return Err(bad(
                    table,
                    Some(column),
                    TableDefError::ReservedName {
                        name: column.clone(),
                    },
                ));
            }
        }
        let table = &mut tables[index];
        table.parent = ancestors.last().copied();
        table.ancestors = ancestors;
    }
    Ok(())
}

/// Validates a loop's `control`; `has_end` says whether the loop declares an end segment.
fn compile_control(raw: &RawControl, has_end: bool) -> Result<Control, ControlError> {
    if !has_end {
        return Err(ControlError::NoEnd);
    }
    let positions = [
        ("opener_element", raw.opener_element),
        ("closer_element", raw.closer_element),
        ("count_element", raw.count_element),
    ];
    if let Some((key, _)) = positions.iter().find(|(_, position)| *position == 0) {
        return Err(ControlError::ZeroPosition { key });
    }
    let count = match raw.count.as_str() {
        "segments" => ControlCount::Segments,
        "children" => ControlCount::Children,
        _ => {
            return Err(ControlError::UnknownCount {
                found: raw.count.clone(),
            });
        }
    };
    Ok(Control {
        opener_element: raw.opener_element,
        closer_element: raw.closer_element,
        count_element: raw.count_element,
        count,
    })
}

/// A trigger as `"N1" where {1: "PR", 2: "X"}`, or `"N1" with no conditions`.
pub(crate) fn render_trigger(trigger: &Trigger) -> String {
    let segment = String::from_utf8_lossy(&trigger.segment);
    if trigger.conditions.is_empty() {
        return format!("{segment:?} with no conditions");
    }
    let parts: Vec<String> = trigger
        .conditions
        .iter()
        .map(|(position, value)| format!("{position}: {:?}", String::from_utf8_lossy(value)))
        .collect();
    format!("{segment:?} where {{{}}}", parts.join(", "))
}

/// A 1-based element position written in canonical form (`"1"`, never
/// `"01"` or `"+1"`), so no two keys can name the same position.
fn parse_position(key: &str) -> Option<usize> {
    key.parse::<usize>()
        .ok()
        .filter(|&p| p >= 1 && p.to_string() == key)
}

/// Compiles the elements of `segment`, or the components of the element at
/// `parent` (its key as written) when one is given.
fn compile_elements(
    segment: &str,
    raw: &BTreeMap<String, RawElement>,
    parent: Option<&str>,
) -> Result<BTreeMap<usize, ElementDef>, SpecError> {
    let mut elements = BTreeMap::new();
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, def) in raw {
        let position_text = match parent {
            Some(parent) => format!("{parent}.composite.{key}"),
            None => key.clone(),
        };
        let fail = |reason| SpecError::BadElementDef {
            segment: segment.to_string(),
            position: position_text.clone(),
            reason,
        };
        let position =
            parse_position(key).ok_or_else(|| fail(ElementDefError::NonCanonicalPosition))?;
        if def.name.is_empty() {
            return Err(fail(ElementDefError::EmptyName));
        }
        if let Some(first) = names.insert(def.name.as_str(), key.as_str()) {
            return Err(fail(ElementDefError::DuplicateName {
                name: def.name.clone(),
                first: first.to_string(),
            }));
        }
        let kind = ElementType::parse(&def.kind, def.scale).map_err(fail)?;
        if let ElementType::R { scale } = kind
            && scale > ElementType::MAX_SCALE
        {
            return Err(fail(ElementDefError::ScaleAboveMaximum { scale }));
        }
        if def.max == Some(0) {
            return Err(fail(ElementDefError::ZeroMax));
        }
        if let (Some(min), Some(max)) = (def.min, def.max)
            && min > max
        {
            return Err(fail(ElementDefError::MinAboveMax { min, max }));
        }
        let composite = if def.composite.is_empty() {
            BTreeMap::new()
        } else if parent.is_some() {
            return Err(fail(ElementDefError::NestedComposite));
        } else if kind != ElementType::An {
            return Err(fail(ElementDefError::CompositeOnNonAn {
                kind: def.kind.clone(),
            }));
        } else {
            compile_elements(segment, &def.composite, Some(key))?
        };
        elements.insert(
            position,
            ElementDef {
                name: def.name.clone(),
                kind,
                required: def.required,
                min: def.min,
                max: def.max,
                composite,
            },
        );
    }
    Ok(elements)
}

/// How a JSON value is described when it is not the object a spec expects.
fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// The value as an object, or [`SpecError::NotAnObject`] naming `path`.
fn object_at<'v>(
    value: &'v Value,
    path: &str,
) -> Result<&'v serde_json::Map<String, Value>, SpecError> {
    value.as_object().ok_or_else(|| SpecError::NotAnObject {
        path: path.to_string(),
        found: kind_of(value),
    })
}

/// The scalar kinds the raw structs expect.
#[derive(Clone, Copy)]
enum Leaf {
    Text,
    Flag,
    Count,
    Byte,
}

impl Leaf {
    fn expected(self) -> &'static str {
        match self {
            Leaf::Text => "a string",
            Leaf::Flag => "a boolean",
            Leaf::Count => "a non-negative integer",
            Leaf::Byte => "an integer from 0 to 255",
        }
    }

    /// What `value` is, in words, when it is not this kind; `None` when it is.
    fn mismatch(self, value: &Value) -> Option<&'static str> {
        match (self, value) {
            (Leaf::Text, Value::String(_)) | (Leaf::Flag, Value::Bool(_)) => None,
            (Leaf::Count | Leaf::Byte, Value::Number(number)) => {
                if let Some(n) = number.as_u64() {
                    if matches!(self, Leaf::Byte) && n > 255 {
                        Some("a number above 255")
                    } else {
                        None
                    }
                } else if number.is_f64() {
                    Some("a fractional number")
                } else {
                    Some("a negative number")
                }
            }
            _ => Some(kind_of(value)),
        }
    }
}

/// A key as it appears in a rendered path: unchanged, or JSON-quoted when it
/// is empty or holds a path separator (`.`, `/`, `#`) or whitespace, which
/// would make the path ambiguous.
pub(crate) fn render_key(key: &str) -> Cow<'_, str> {
    let ambiguous = key.is_empty()
        || key
            .chars()
            .any(|c| matches!(c, '.' | '/' | '#') || c.is_whitespace());
    if !ambiguous {
        return Cow::Borrowed(key);
    }
    Cow::Owned(serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}")))
}

/// Joins a key to the path it sits under.
fn child(at: &str, key: &str) -> String {
    let key = render_key(key);
    if at.is_empty() {
        key.into_owned()
    } else {
        format!("{at}.{key}")
    }
}

/// Requires `value` to be of the `kind`.
fn check_leaf(value: &Value, at: &str, kind: Leaf) -> Result<(), SpecError> {
    match kind.mismatch(value) {
        None => Ok(()),
        Some(found) => Err(SpecError::WrongType {
            path: at.to_string(),
            expected: kind.expected(),
            found,
        }),
    }
}

/// Requires the member `key` of `map`, when present, to be of the `kind`;
/// `null` also passes when `nullable`.
fn check_member(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
    kind: Leaf,
    nullable: bool,
) -> Result<(), SpecError> {
    match map.get(key) {
        None => Ok(()),
        Some(Value::Null) if nullable => Ok(()),
        Some(value) => check_leaf(value, &child(at, key), kind),
    }
}

/// Requires the member `key` of `map`, when present, to be an object whose
/// values are all of the `kind`.
fn check_member_map(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
    kind: Leaf,
) -> Result<(), SpecError> {
    if let Some(members) = map.get(key) {
        let at = child(at, key);
        for (name, value) in object_at(members, &at)? {
            check_leaf(value, &child(&at, name), kind)?;
        }
    }
    Ok(())
}

/// Requires the member `key` of `map`, when present, to be an array of strings.
fn check_member_texts(
    map: &serde_json::Map<String, Value>,
    at: &str,
    key: &str,
) -> Result<(), SpecError> {
    let Some(list) = map.get(key) else {
        return Ok(());
    };
    let at = child(at, key);
    let Value::Array(items) = list else {
        return Err(SpecError::WrongType {
            path: at,
            expected: "an array of strings",
            found: kind_of(list),
        });
    };
    for (i, item) in items.iter().enumerate() {
        check_leaf(item, &child(&at, &i.to_string()), Leaf::Text)?;
    }
    Ok(())
}

/// Requires an object everywhere the schema has one, and the scalar kind
/// everywhere the schema has a scalar, before serde sees the value: serde
/// would accept an array in place of a struct, and its messages for the wrong
/// kind of value name neither the key nor, for objects, that an object was
/// expected. Missing and unknown keys are left to the schema.
fn check_shape(source: &Value) -> Result<(), SpecError> {
    let root = object_at(source, "")?;
    check_member(root, "", "name", Leaf::Text, false)?;
    if let Some(loops) = root.get("loops") {
        for (name, def) in object_at(loops, "loops")? {
            let at = child("loops", name);
            let def = object_at(def, &at)?;
            check_member(def, &at, "parent", Leaf::Text, true)?;
            check_member(def, &at, "end", Leaf::Text, true)?;
            check_member_texts(def, &at, "segments")?;
            if let Some(trigger) = def.get("trigger") {
                let at = child(&at, "trigger");
                let trigger = object_at(trigger, &at)?;
                check_member(trigger, &at, "segment", Leaf::Text, false)?;
                if let Some(conditions) = trigger.get("where") {
                    object_at(conditions, &child(&at, "where"))?;
                }
                check_member_map(trigger, &at, "where", Leaf::Text)?;
            }
            if let Some(control) = def.get("control") {
                let at = child(&at, "control");
                let control = object_at(control, &at)?;
                for key in ["opener_element", "closer_element", "count_element"] {
                    check_member(control, &at, key, Leaf::Count, false)?;
                }
                check_member(control, &at, "count", Leaf::Text, false)?;
            }
        }
    }
    if let Some(tables) = root.get("tables") {
        for (name, def) in object_at(tables, "tables")? {
            let at = child("tables", name);
            let def = object_at(def, &at)?;
            check_member_texts(def, &at, "loops")?;
            check_member(def, &at, "ref", Leaf::Text, true)?;
            check_member(def, &at, "segment", Leaf::Text, true)?;
            if let Some(repeat) = def.get("repeat") {
                let at = child(&at, "repeat");
                let repeat = object_at(repeat, &at)?;
                check_member(repeat, &at, "from", Leaf::Count, false)?;
                check_member(repeat, &at, "step", Leaf::Count, false)?;
            }
            if let Some(columns) = def.get("columns") {
                let at = child(&at, "columns");
                for (column, def) in object_at(columns, &at)? {
                    let at = child(&at, column);
                    let def = object_at(def, &at)?;
                    check_member(def, &at, "loop", Leaf::Text, true)?;
                    check_member(def, &at, "segment", Leaf::Text, true)?;
                    if let Some(conditions) = def.get("where") {
                        object_at(conditions, &child(&at, "where"))?;
                    }
                    check_member_map(def, &at, "where", Leaf::Text)?;
                    for key in ["element", "component", "group_element"] {
                        check_member(def, &at, key, Leaf::Count, true)?;
                    }
                    check_member(def, &at, "segment_index", Leaf::Flag, false)?;
                }
            }
        }
    }
    if let Some(segments) = root.get("segments") {
        for (id, def) in object_at(segments, "segments")? {
            let at = child("segments", id);
            let def = object_at(def, &at)?;
            if let Some(elements) = def.get("elements") {
                check_elements_shape(elements, &child(&at, "elements"))?;
            }
        }
    }
    Ok(())
}

/// Requires every element (and every component) definition to be an object
/// whose scalar members are of the kind the schema expects.
fn check_elements_shape(elements: &Value, at: &str) -> Result<(), SpecError> {
    for (position, def) in object_at(elements, at)? {
        let at = child(at, position);
        let def = object_at(def, &at)?;
        check_member(def, &at, "name", Leaf::Text, false)?;
        check_member(def, &at, "type", Leaf::Text, false)?;
        check_member(def, &at, "required", Leaf::Flag, false)?;
        check_member(def, &at, "min", Leaf::Count, true)?;
        check_member(def, &at, "max", Leaf::Count, true)?;
        check_member(def, &at, "scale", Leaf::Byte, true)?;
        if let Some(composite) = def.get("composite") {
            check_elements_shape(composite, &child(&at, "composite"))?;
        }
    }
    Ok(())
}

/// Fails with the first parent cycle found, walking from each loop in spec
/// order. The members start at the loop the walk reaches twice.
fn check_cycles(loops: &[LoopDef]) -> Result<(), SpecError> {
    for start in 0..loops.len() {
        let mut walk: Vec<LoopId> = Vec::new();
        let mut current = Some(LoopId(start));
        while let Some(id) = current {
            if let Some(repeat) = walk.iter().position(|&seen| seen == id) {
                let members = walk[repeat..]
                    .iter()
                    .map(|&member| loops[member.0].name.clone())
                    .collect();
                return Err(SpecError::Cycle { members });
            }
            walk.push(id);
            current = loops[id.0].parent;
        }
    }
    Ok(())
}

/// JSON Merge Patch, RFC 7386: `patch` objects merge into `target` key by key,
/// `null` members delete, and anything that is not an object replaces `target`.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(members) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let Value::Object(target) = target {
        for (key, value) in members {
            if value.is_null() {
                target.remove(key);
            } else {
                merge_patch(target.entry(key.as_str()).or_insert(Value::Null), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Delimiters, Tokenizer};

    fn segs(input: &[u8]) -> Vec<Segment<'_>> {
        Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')).collect()
    }

    #[test]
    fn builtin_835_loads() {
        let spec = Spec::builtin_835();
        assert_eq!(spec.name(), "835");
        assert_eq!(spec.loops().len(), 8);
        let interchange = spec.loop_id("interchange").unwrap();
        assert_eq!(spec.roots(), &[interchange]);
        let transaction = spec.loop_id("transaction").unwrap();
        let names: Vec<_> = spec
            .children(Some(transaction))
            .iter()
            .map(|&c| spec.loop_name(c))
            .collect();
        assert_eq!(names, vec!["1000A", "1000B", "2000"]);
        assert_eq!(
            spec.get(spec.loop_id("2110").unwrap()).parent,
            spec.loop_id("2100")
        );
        assert_eq!(spec.get(transaction).end.as_deref(), Some(&b"SE"[..]));
    }

    #[test]
    fn builtin_835_defines_every_segment_its_loops_name() {
        let spec = Spec::builtin_835();
        for def in spec.loops() {
            let ids = std::iter::once(&def.trigger.segment)
                .chain(&def.segments)
                .chain(def.end.as_ref());
            for id in ids {
                assert!(
                    spec.segment(id).is_some(),
                    "loop {} names {} with no definition",
                    def.name,
                    String::from_utf8_lossy(id)
                );
            }
        }
        let ids: Vec<String> = spec
            .segments()
            .map(|(id, _)| String::from_utf8_lossy(id).into_owned())
            .collect();
        assert_eq!(
            ids,
            vec![
                "AMT", "BPR", "CAS", "CLP", "CUR", "DTM", "GE", "GS", "IEA", "ISA", "LQ", "LX",
                "MIA", "MOA", "N1", "N3", "N4", "NM1", "PER", "PLB", "QTY", "RDM", "REF", "SE",
                "ST", "SVC", "TRN", "TS2", "TS3"
            ]
        );
    }

    #[test]
    fn builtin_835_element_names_are_unique_among_siblings() {
        fn check(at: &str, elements: &BTreeMap<usize, ElementDef>) {
            let mut seen = std::collections::BTreeSet::new();
            for (position, def) in elements {
                assert!(
                    seen.insert(def.name.as_str()),
                    "{at}: name {} repeats at position {position}",
                    def.name
                );
                check(&format!("{at}{position:02}"), &def.composite);
            }
        }
        for (id, def) in Spec::builtin_835().segments() {
            check(&String::from_utf8_lossy(id), &def.elements);
        }
    }

    #[test]
    fn builtin_835_types_the_elements_the_envelope_checks_rely_on() {
        let spec = Spec::builtin_835();
        let element = |id: &[u8], position: usize| &spec.segment(id).unwrap().elements[&position];
        assert_eq!(element(b"CLP", 1).name, "claim_submitter_identifier");
        assert_eq!(element(b"CLP", 3).kind, ElementType::R { scale: 2 });
        assert_eq!(element(b"SE", 1).kind, ElementType::N(0));
        assert_eq!(element(b"ISA", 13).kind, ElementType::N(0));
        assert_eq!(element(b"DTM", 2).kind, ElementType::Dt);
        let procedure = element(b"SVC", 1);
        assert!(procedure.required);
        assert_eq!(procedure.composite.len(), 8);
        assert_eq!(procedure.composite[&2].name, "procedure_code");
    }

    #[test]
    fn builtin_835_declares_five_tables_and_how_they_nest() {
        let spec = Spec::builtin_835();
        let names: Vec<&str> = spec.tables().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "adjustments",
                "claims",
                "payments",
                "provider_adjustments",
                "services"
            ]
        );
        let above = |name: &str| -> Vec<&str> {
            spec.table(name)
                .unwrap()
                .ancestors
                .iter()
                .map(|&i| spec.tables()[i].name.as_str())
                .collect()
        };
        assert!(above("payments").is_empty());
        assert_eq!(above("claims"), vec!["payments"]);
        assert_eq!(above("services"), vec!["payments", "claims"]);
        assert_eq!(above("adjustments"), vec!["payments", "claims", "services"]);
        assert_eq!(above("provider_adjustments"), vec!["payments"]);
        let references: Vec<&str> = ["payments", "claims", "services"]
            .iter()
            .map(|name| spec.table(name).unwrap().reference.as_str())
            .collect();
        assert_eq!(references, vec!["payment", "claim", "service"]);
        let loops = |name: &str| -> Vec<&str> {
            spec.table(name)
                .unwrap()
                .loops
                .iter()
                .map(|&id| spec.loop_name(id))
                .collect()
        };
        assert_eq!(loops("adjustments"), vec!["2100", "2110"]);
        assert_eq!(loops("provider_adjustments"), vec!["transaction"]);
        let adjustments = spec.table("adjustments").unwrap();
        assert_eq!(adjustments.segment.as_deref(), Some(&b"CAS"[..]));
        assert_eq!(adjustments.repeat, Some(Repeat { from: 2, step: 3 }));
        let plb = spec.table("provider_adjustments").unwrap();
        assert_eq!(plb.segment.as_deref(), Some(&b"PLB"[..]));
        assert_eq!(plb.repeat, Some(Repeat { from: 3, step: 2 }));
        let counts: Vec<usize> = spec.tables().iter().map(|t| t.columns.len()).collect();
        assert_eq!(counts, vec![4, 21, 13, 5, 9]);
    }

    #[test]
    fn builtin_835_columns_read_elements_the_spec_defines_in_loops_that_hold_them() {
        let spec = Spec::builtin_835();
        for table in spec.tables() {
            for (column, source) in &table.columns {
                let at = format!("{}.{column}", table.name);
                match source {
                    ColumnSource::Element {
                        loop_id,
                        segment,
                        element,
                        component,
                        ..
                    } => {
                        assert!(
                            spec.element_def(segment, *element, *component).is_some(),
                            "{at} reads an element the spec does not define"
                        );
                        let readers = loop_id.map_or(table.loops.clone(), |id| vec![id]);
                        for id in readers {
                            let def = spec.get(id);
                            assert!(
                                def.trigger.segment == *segment || def.accepts(segment),
                                "{at}: loop {} does not hold {}",
                                def.name,
                                String::from_utf8_lossy(segment)
                            );
                        }
                    }
                    ColumnSource::SegmentIndex { .. } => {}
                    ColumnSource::GroupElement { offset, component } => {
                        let (Some(segment), Some(repeat)) = (&table.segment, table.repeat) else {
                            panic!("{at}: a group column outside a repeating table");
                        };
                        assert!(
                            spec.element_def(segment, repeat.from + offset, *component)
                                .is_some(),
                            "{at} reads a group element the spec does not define"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ancestors_are_listed_root_first() {
        let spec = Spec::builtin_835();
        let chain: Vec<_> = spec
            .ancestors(spec.loop_id("2110").unwrap())
            .iter()
            .map(|&c| spec.loop_name(c))
            .collect();
        assert_eq!(
            chain,
            vec!["interchange", "group", "transaction", "2000", "2100"]
        );
        assert!(
            spec.ancestors(spec.loop_id("interchange").unwrap())
                .is_empty()
        );
    }

    #[test]
    fn trigger_conditions_are_checked_against_elements() {
        let spec = Spec::builtin_835();
        let segments = segs(b"N1*PR*PAYER~N1*PE*PAYEE~N1*TT*OTHER~");
        let transaction = spec.loop_id("transaction");
        assert_eq!(
            spec.matching_child(transaction, &segments[0]),
            spec.loop_id("1000A")
        );
        assert_eq!(
            spec.matching_child(transaction, &segments[1]),
            spec.loop_id("1000B")
        );
        assert_eq!(spec.matching_child(transaction, &segments[2]), None);
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            None,
            "N1 is not a root trigger"
        );
    }

    #[test]
    fn matching_any_finds_a_loop_regardless_of_parent() {
        let spec = Spec::builtin_835();
        let segments = segs(b"SVC*HC:1*10*10~ZZZ*1~");
        assert_eq!(spec.matching_any(&segments[0]), spec.loop_id("2110"));
        assert_eq!(spec.matching_any(&segments[1]), None);
    }

    #[test]
    fn more_specific_trigger_wins_over_a_bare_one() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "any":{"trigger":{"segment":"N1"}},
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        )
        .unwrap();
        let segments = segs(b"N1*PR~N1*PE~");
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            spec.loop_id("payer")
        );
        assert_eq!(spec.matching_child(None, &segments[1]), spec.loop_id("any"));
    }

    #[test]
    fn accepts_covers_segments_and_end() {
        let spec = Spec::builtin_835();
        let transaction = spec.get(spec.loop_id("transaction").unwrap());
        assert!(transaction.accepts(b"BPR"));
        assert!(transaction.accepts(b"SE"));
        assert!(!transaction.accepts(b"CLP"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err =
            Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"typo":1}}}"#)
                .unwrap_err();
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "a"),
            "{err}"
        );
    }

    #[test]
    fn unknown_parent_is_rejected_with_both_names() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"parent":"ghost","trigger":{"segment":"AA"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::UnknownParent { loop_name, parent } if loop_name == "a" && parent == "ghost"),
            "{err}"
        );
    }

    #[test]
    fn parent_cycle_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"parent":"b","trigger":{"segment":"AA"}},
                "b":{"parent":"a","trigger":{"segment":"BB"}}
            }}"#,
        )
        .unwrap_err();
        assert!(matches!(err, SpecError::Cycle { .. }), "{err}");
    }

    #[test]
    fn cycle_lists_its_members_from_the_repeated_loop() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"parent":"b","trigger":{"segment":"AA"}},
                "b":{"parent":"c","trigger":{"segment":"BB"}},
                "c":{"parent":"b","trigger":{"segment":"CC"}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Cycle { members } if members == &["b", "c"]),
            "{err:?}"
        );
        assert_eq!(err.to_string(), "loops form a parent cycle: b -> c -> b");
    }

    #[test]
    fn a_loop_that_is_its_own_parent_is_a_cycle() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"parent":"a","trigger":{"segment":"AA"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Cycle { members } if members == &["a"]),
            "{err:?}"
        );
        assert_eq!(err.to_string(), "loops form a parent cycle: a -> a");
    }

    #[test]
    fn a_schema_error_names_the_loop() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "2000":{"trigger":{"segment":"LX"}},
                "2100":{"parent":"2000","trigger":{"segment":"CLP"},"segmnts":["DTM"]}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "2100"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("loop \"2100\" does not match the schema: "),
            "{err}"
        );
        assert!(err.to_string().contains("segmnts"), "{err}");
    }

    #[test]
    fn a_malformed_top_level_is_a_schema_error_without_a_loop() {
        let err = Spec::from_json(r#"{"name":"t"}"#).unwrap_err();
        assert!(
            matches!(
                &err,
                SpecError::Schema {
                    loop_name: None,
                    ..
                }
            ),
            "{err:?}"
        );
        let message = err.to_string();
        assert!(
            message.starts_with("spec does not match the schema: "),
            "{message}"
        );
        assert!(!message.contains("RawSpec"), "{message}");
    }

    #[test]
    fn a_spec_that_is_not_an_object_says_so_in_plain_words() {
        let err = Spec::from_json("[]").unwrap_err();
        assert!(
            matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path.is_empty()),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found an array"
        );
    }

    #[test]
    fn every_object_of_the_loop_schema_is_checked_with_its_path() {
        let cases = [
            (r#"{"name":"t","loops":[]}"#, "loops", "an array"),
            (r#"{"name":"t","loops":{"a":"AA"}}"#, "loops.a", "a string"),
            (
                r#"{"name":"t","loops":{"a":{"trigger":["AA"]}}}"#,
                "loops.a.trigger",
                "an array",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":1}}}}"#,
                "loops.a.trigger.where",
                "a number",
            ),
            (
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":null}}}}"#,
                "loops.a.trigger.where",
                "null",
            ),
        ];
        for (json, expected_path, expected_found) in cases {
            let err = Spec::from_json(json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{json}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patched_spec_goes_through_the_same_shape_check() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2100":{"trigger":["CLP"]}}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
    }

    #[test]
    fn keys_with_a_separator_or_whitespace_are_quoted_in_paths() {
        assert_eq!(render_key("2100"), "2100");
        assert_eq!(render_key("a.b"), "\"a.b\"");
        assert_eq!(render_key("x/y"), "\"x/y\"");
        assert_eq!(render_key("x#2"), "\"x#2\"");
        assert_eq!(render_key("a b"), "\"a b\"");
        assert_eq!(render_key(""), "\"\"");
    }

    #[test]
    fn a_spec_error_path_quotes_a_loop_name_that_holds_a_dot() {
        let plain = Spec::from_json(r#"{"name":"t","loops":{"ab":{"trigger":[]}}}"#).unwrap_err();
        assert!(
            matches!(&plain, SpecError::NotAnObject { path, .. } if path == "loops.ab.trigger"),
            "{plain:?}"
        );
        let quoted = Spec::from_json(r#"{"name":"t","loops":{"a.b":{"trigger":[]}}}"#).unwrap_err();
        assert!(
            matches!(&quoted, SpecError::NotAnObject { path, .. } if path == "loops.\"a.b\".trigger"),
            "{quoted:?}"
        );
        let wrong = Spec::from_json(
            r#"{"name":"t","loops":{"a b":{"trigger":{"segment":"AA","where":{"1":2}}}}}"#,
        )
        .unwrap_err();
        assert_eq!(
            wrong.to_string(),
            "spec: the value at loops.\"a b\".trigger.where.1 must be a string; found a number"
        );
    }

    #[test]
    fn an_empty_segment_id_path_quotes_a_table_or_column_name_that_holds_a_dot() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"A":{"trigger":{"segment":"AA"}}},
                "tables":{"a.b":{"loops":["A"],"columns":{"c d":{"segment":"","element":1}}}}}"#,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "the spec has an empty segment id at tables.\"a.b\".columns.\"c d\".segment"
        );
    }

    #[test]
    fn every_kind_of_json_value_is_named() {
        use serde_json::json;
        assert_eq!(kind_of(&json!(null)), "null");
        assert_eq!(kind_of(&json!(true)), "a boolean");
        assert_eq!(kind_of(&json!(1)), "a number");
        assert_eq!(kind_of(&json!("x")), "a string");
        assert_eq!(kind_of(&json!([])), "an array");
        assert_eq!(kind_of(&json!({})), "an object");
    }

    #[test]
    fn a_spec_without_loops_is_rejected() {
        let err = Spec::from_json(r#"{"name":"t","loops":{}}"#).unwrap_err();
        assert!(
            matches!(&err, SpecError::NoLoops { spec_name } if spec_name == "t"),
            "{err:?}"
        );
    }

    #[test]
    fn several_top_level_loops_are_allowed() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}},"b":{"trigger":{"segment":"BB"}}}}"#,
        )
        .unwrap();
        assert_eq!(spec.roots().len(), 2);
    }

    #[test]
    fn empty_segment_ids_are_rejected_with_the_loop_and_the_key() {
        let cases = [
            (r#"{"trigger":{"segment":""}}"#, "trigger.segment"),
            (
                r#"{"trigger":{"segment":"AA"},"segments":["A1",""]}"#,
                "segments[1]",
            ),
            (r#"{"trigger":{"segment":"AA"},"end":""}"#, "end"),
        ];
        for (def, expected_key) in cases {
            let json = format!(r#"{{"name":"t","loops":{{"a":{def}}}}}"#);
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::EmptySegmentId { loop_name: Some(name), key } if name == "a" && key == expected_key),
                "{def}: {err:?}"
            );
        }
    }

    #[test]
    fn an_empty_segment_id_in_the_segments_section_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"":{"elements":{}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == "segments.\"\""),
            "{err:?}"
        );
    }

    #[test]
    fn non_numeric_zero_or_non_canonical_positions_are_rejected() {
        for position in ["x", "0", "-1", "01", "+1", " 1"] {
            let json = format!(
                r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA","where":{{"{position}":"1"}}}}}}}}}}"#
            );
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::BadPosition { loop_name, position: p } if loop_name == "a" && p == position),
                "{err}"
            );
        }
    }

    #[test]
    fn a_position_spelled_twice_is_rejected_by_its_non_canonical_spelling() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA","where":{"1":"X","01":"Y"}}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::BadPosition { loop_name, position } if loop_name == "a" && position == "01"),
            "{err:?}"
        );
    }

    #[test]
    fn ambiguous_trigger_message_shows_the_parent_and_the_trigger() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "a":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "b":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\"}"
        );
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA"}},
                "b":{"trigger":{"segment":"AA"}}
            }}"#,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
        );
    }

    #[test]
    fn ambiguous_sibling_triggers_are_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA","where":{"1":"X"}}},
                "b":{"trigger":{"segment":"AA","where":{"1":"X"}}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::AmbiguousTrigger { first, second, parent: None, segment, .. } if first == "a" && second == "b" && segment == "AA"),
            "{err}"
        );
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{
                "a":{"trigger":{"segment":"AA","where":{"1":"X"}}},
                "b":{"trigger":{"segment":"AA","where":{"1":"Y"}}}
            }}"#,
        );
        assert!(ok.is_ok(), "different conditions are not ambiguous");
    }

    #[test]
    fn siblings_testing_different_positions_overlap_and_are_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}},
                "other":{"parent":"transaction","trigger":{"segment":"N1","where":{"2":"X"}}}
            }}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::OverlappingTriggers { parent: Some(parent), a, b, .. } if parent == "transaction" && a == "other" && b == "payer"),
            "{err:?}"
        );
        assert_eq!(
            err.to_string(),
            "loops \"other\" and \"payer\" under \"transaction\" can open on the same segment: \"other\" on \"N1\" where {2: \"X\"}, \"payer\" on \"N1\" where {1: \"PR\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn a_bare_trigger_beside_a_conditioned_sibling_is_a_catch_all_and_loads() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "transaction":{"trigger":{"segment":"ST"}},
                "any":{"parent":"transaction","trigger":{"segment":"N1"}},
                "payer":{"parent":"transaction","trigger":{"segment":"N1","where":{"1":"PR"}}}
            }}"#,
        );
        assert!(spec.is_ok(), "{spec:?}");
    }

    #[test]
    fn a_strict_superset_of_conditions_does_not_overlap() {
        let spec = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR"}}},
                "acme":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}}
            }}"#,
        )
        .unwrap();
        let segments = segs(b"N1*PR*X~N1*PR*Y~");
        assert_eq!(
            spec.matching_child(None, &segments[0]),
            spec.loop_id("acme"),
            "the more specific trigger wins"
        );
        assert_eq!(
            spec.matching_child(None, &segments[1]),
            spec.loop_id("payer")
        );
    }

    #[test]
    fn siblings_that_differ_at_a_shared_position_do_not_overlap() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{
                "payer":{"trigger":{"segment":"N1","where":{"1":"PR","2":"X"}}},
                "payee":{"trigger":{"segment":"N1","where":{"1":"PE"}}},
                "other":{"trigger":{"segment":"N3"}}
            }}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
        let builtin = Spec::builtin_835();
        assert!(builtin.loop_id("1000A").is_some() && builtin.loop_id("1000B").is_some());
    }

    #[test]
    fn overlapping_triggers_display_both_loops_and_their_triggers() {
        let err = SpecError::OverlappingTriggers {
            parent: Some("transaction".into()),
            a: "1000A".into(),
            b: "1000C".into(),
            conditions_a: "\"N1\" where {1: \"PR\"}".into(),
            conditions_b: "\"N1\" where {2: \"X\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"1000A\" and \"1000C\" under \"transaction\" can open on the same segment: \"1000A\" on \"N1\" where {1: \"PR\"}, \"1000C\" on \"N1\" where {2: \"X\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::OverlappingTriggers {
            parent: None,
            a: "a".into(),
            b: "b".into(),
            conditions_a: "\"AA\" where {1: \"X\"}".into(),
            conditions_b: "\"AA\" where {2: \"Y\"}".into(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root can open on the same segment: \"a\" on \"AA\" where {1: \"X\"}, \"b\" on \"AA\" where {2: \"Y\"}, no position they both test requires different values, and neither trigger is more specific than the other"
        );
    }

    #[test]
    fn triggers_render_with_their_conditions_in_position_order() {
        let bare = Trigger {
            segment: b"N1".to_vec(),
            conditions: Vec::new(),
        };
        assert_eq!(render_trigger(&bare), "\"N1\" with no conditions");
        let conditioned = Trigger {
            segment: b"N1".to_vec(),
            conditions: vec![(1, b"PR".to_vec()), (3, b"X".to_vec())],
        };
        assert_eq!(
            render_trigger(&conditioned),
            "\"N1\" where {1: \"PR\", 3: \"X\"}"
        );
    }

    #[test]
    fn to_json_round_trips_through_from_json() {
        let spec = Spec::builtin_835();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert_eq!(again.loops(), spec.loops());
    }

    fn json_error(text: &str) -> serde_json::Error {
        serde_json::from_str::<Value>(text).unwrap_err()
    }

    #[test]
    fn json_error_displays_the_parser_message() {
        let err = SpecError::Json(json_error("{"));
        assert_eq!(
            err.to_string(),
            "invalid spec JSON: EOF while parsing an object at line 1 column 1"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn schema_error_displays_the_loop_and_the_serde_message() {
        let err = SpecError::Schema {
            loop_name: Some("2100".into()),
            source: serde_json::from_value::<RawLoop>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" does not match the schema: invalid type: integer `1`, expected a loop object"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn schema_error_without_a_loop_displays_the_spec() {
        let err = SpecError::Schema {
            loop_name: None,
            source: serde_json::from_value::<RawSpec>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "spec does not match the schema: invalid type: integer `1`, expected a spec object with \"name\" and \"loops\""
        );
    }

    #[test]
    fn patch_error_displays_the_inner_error_once() {
        let err = SpecError::Patch {
            source: Box::new(SpecError::EmptySegmentId {
                loop_name: Some("a".into()),
                key: "end".into(),
            }),
        };
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"a\" has an empty segment id at end"
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"a\" has an empty segment id at end")
        );
    }

    #[test]
    fn not_an_object_displays_the_path_and_what_was_found() {
        let err = SpecError::NotAnObject {
            path: "loops.2100.trigger".into(),
            found: "an array",
        };
        assert_eq!(
            err.to_string(),
            "spec: the value at loops.2100.trigger must be a JSON object; found an array"
        );
        assert!(std::error::Error::source(&err).is_none());
        let err = SpecError::NotAnObject {
            path: String::new(),
            found: "a string",
        };
        assert_eq!(
            err.to_string(),
            "the spec must be a JSON object; found a string"
        );
    }

    #[test]
    fn unknown_parent_displays_both_names() {
        let err = SpecError::UnknownParent {
            loop_name: "2100".into(),
            parent: "2000".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" names unknown parent \"2000\""
        );
    }

    #[test]
    fn cycle_displays_the_walk() {
        let err = SpecError::Cycle {
            members: vec!["b".into(), "c".into()],
        };
        assert_eq!(err.to_string(), "loops form a parent cycle: b -> c -> b");
    }

    #[test]
    fn no_loops_displays_the_spec_name() {
        let err = SpecError::NoLoops {
            spec_name: "name".into(),
        };
        assert_eq!(err.to_string(), "spec \"name\" declares no loops");
    }

    #[test]
    fn empty_segment_id_displays_the_loop_and_the_key() {
        let err = SpecError::EmptySegmentId {
            loop_name: Some("2100".into()),
            key: "segments[3]".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"2100\" has an empty segment id at segments[3]"
        );
        let err = SpecError::EmptySegmentId {
            loop_name: None,
            key: "segments.\"\"".into(),
        };
        assert_eq!(
            err.to_string(),
            "the spec has an empty segment id at segments.\"\""
        );
    }

    #[test]
    fn a_loop_ending_on_its_own_trigger_is_rejected() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"end":"AA"}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::EndIsTrigger { loop_name, segment } if loop_name == "a" && segment == "AA"),
            "{err:?}"
        );
        assert!(
            Spec::from_json(
                r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"},"end":"BB"}}}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn end_is_trigger_displays_the_loop_the_segment_and_why() {
        let err = SpecError::EndIsTrigger {
            loop_name: "a".into(),
            segment: "AA".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"a\" has \"end\" \"AA\", the same segment as its \"trigger\": the loop would close on the segment that opens it"
        );
    }

    #[test]
    fn bad_position_displays_the_key_and_the_rule() {
        let err = SpecError::BadPosition {
            loop_name: "a".into(),
            position: "01".into(),
        };
        assert_eq!(
            err.to_string(),
            "loop \"a\" has an invalid \"where\" position \"01\": positions are 1-based integers written in canonical form"
        );
    }

    #[test]
    fn ambiguous_trigger_displays_parent_segment_and_conditions() {
        let err = SpecError::AmbiguousTrigger {
            first: "a".into(),
            second: "b".into(),
            parent: Some("transaction".into()),
            segment: "N1".into(),
            conditions: vec![(1, "PR".into()), (3, "XX".into())],
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under \"transaction\" share the identical trigger \"N1\" where {1: \"PR\", 3: \"XX\"}"
        );
        let err = SpecError::AmbiguousTrigger {
            first: "a".into(),
            second: "b".into(),
            parent: None,
            segment: "AA".into(),
            conditions: Vec::new(),
        };
        assert_eq!(
            err.to_string(),
            "loops \"a\" and \"b\" under the root share the identical trigger \"AA\""
        );
    }

    const CLP_ONLY: &str = r#"{"name":"t",
        "loops":{"2100":{"trigger":{"segment":"CLP"},"segments":["ZZ1"]}},
        "segments":{"CLP":{"elements":{
            "1":{"name":"claim_submitter_id","type":"AN","required":true,"min":1,"max":38},
            "3":{"name":"total_claim_charge_amount","type":"R","required":true},
            "12":{"name":"drg_weight","type":"R","scale":4}
        }},
        "SVC":{"elements":{
            "1":{"name":"procedure","type":"AN","required":true,"composite":{
                "1":{"name":"qualifier","type":"ID","required":true,"min":2,"max":2},
                "2":{"name":"code","type":"AN","required":true}
            }},
            "5":{"name":"units","type":"N0"}
        }}}
    }"#;

    fn element_error(elements: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{{"AA":{{"elements":{elements}}}}}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn segments_are_keyed_by_id_and_elements_by_position() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        let first = &clp.elements[&1];
        assert_eq!(first.name, "claim_submitter_id");
        assert_eq!(first.kind, ElementType::An);
        assert!(first.required);
        assert_eq!((first.min, first.max), (Some(1), Some(38)));
        assert_eq!(clp.elements[&3].kind, ElementType::R { scale: 2 });
        assert!(!clp.elements[&12].required, "required defaults to false");
        assert_eq!(clp.elements[&12].kind, ElementType::R { scale: 4 });
        assert_eq!(
            clp.elements.keys().copied().collect::<Vec<_>>(),
            vec![1, 3, 12]
        );
        let svc = spec.segment(b"SVC").unwrap();
        let procedure = &svc.elements[&1];
        assert_eq!(procedure.composite[&1].kind, ElementType::Id);
        assert_eq!(procedure.composite[&2].name, "code");
        assert_eq!(svc.elements[&5].kind, ElementType::N(0));
        let ids: Vec<&[u8]> = spec.segments().map(|(id, _)| id).collect();
        assert_eq!(ids, vec![&b"CLP"[..], &b"SVC"[..]]);
    }

    #[test]
    fn every_type_code_is_read() {
        let cases = [
            ("AN", None, ElementType::An),
            ("ID", None, ElementType::Id),
            ("N0", None, ElementType::N(0)),
            ("N2", None, ElementType::N(2)),
            ("N9", None, ElementType::N(9)),
            ("R", None, ElementType::R { scale: 2 }),
            ("R", Some(6), ElementType::R { scale: 6 }),
            ("DT", None, ElementType::Dt),
            ("TM", None, ElementType::Tm),
        ];
        for (code, scale, expected) in cases {
            assert_eq!(ElementType::parse(code, scale), Ok(expected), "{code}");
        }
        for code in ["an", "N", "N10", "NA", "R2", "", "B"] {
            assert_eq!(
                ElementType::parse(code, None),
                Err(ElementDefError::UnknownType {
                    found: code.to_string()
                }),
                "{code:?}"
            );
        }
    }

    #[test]
    fn element_types_display_their_code_and_meaning() {
        assert_eq!(ElementType::An.to_string(), "AN (string)");
        assert_eq!(ElementType::Id.to_string(), "ID (code)");
        assert_eq!(
            ElementType::N(2).to_string(),
            "N2 (integer with 2 implied decimals)"
        );
        assert_eq!(
            ElementType::R { scale: 2 }.to_string(),
            "R (decimal, scale 2)"
        );
        assert_eq!(ElementType::Dt.to_string(), "DT (date CCYYMMDD or YYMMDD)");
        assert_eq!(
            ElementType::Tm.to_string(),
            "TM (time HHMM, HHMMSS or HHMMSSD..)"
        );
    }

    #[test]
    fn a_segment_a_loop_lists_without_a_definition_stays_opaque() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        assert!(spec.get(spec.loop_id("2100").unwrap()).accepts(b"ZZ1"));
        assert_eq!(spec.segment(b"ZZ1"), None);
    }

    #[test]
    fn a_spec_without_segments_has_none() {
        let spec =
            Spec::from_json(r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}}}"#).unwrap();
        assert_eq!(spec.segments().count(), 0);
    }

    #[test]
    fn bad_element_definitions_are_rejected_with_segment_position_and_reason() {
        let cases = [
            (
                r#"{"01":{"name":"a","type":"AN"}}"#,
                "01",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"0":{"name":"a","type":"AN"}}"#,
                "0",
                ElementDefError::NonCanonicalPosition,
            ),
            (
                r#"{"1":{"name":"","type":"AN"}}"#,
                "1",
                ElementDefError::EmptyName,
            ),
            (
                r#"{"1":{"name":"a","type":"AN"},"2":{"name":"a","type":"ID"}}"#,
                "2",
                ElementDefError::DuplicateName {
                    name: "a".into(),
                    first: "1".into(),
                },
            ),
            (
                r#"{"1":{"name":"a","type":"XX"}}"#,
                "1",
                ElementDefError::UnknownType { found: "XX".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"N2","scale":2}}"#,
                "1",
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","min":5,"max":2}}"#,
                "1",
                ElementDefError::MinAboveMax { min: 5, max: 2 },
            ),
            (
                r#"{"1":{"name":"a","type":"ID","composite":{"1":{"name":"b","type":"AN"}}}}"#,
                "1",
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"2":{"name":"b","type":"AN","composite":{"1":{"name":"c","type":"AN"}}}}}}"#,
                "1.composite.2",
                ElementDefError::NestedComposite,
            ),
            (
                r#"{"1":{"name":"a","type":"AN","composite":{"1":{"name":"b","type":"AN"},"2":{"name":"b","type":"AN"}}}}"#,
                "1.composite.2",
                ElementDefError::DuplicateName {
                    name: "b".into(),
                    first: "1".into(),
                },
            ),
        ];
        for (elements, expected_position, expected_reason) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == expected_position && *reason == expected_reason),
                "{elements}: {err:?}"
            );
        }
    }

    #[test]
    fn names_only_need_to_be_unique_among_siblings() {
        let ok = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{
                "1":{"name":"code","type":"AN","composite":{"1":{"name":"code","type":"AN"}}}
            }}}}"#,
        );
        assert!(ok.is_ok(), "{ok:?}");
    }

    #[test]
    fn a_segment_definition_that_breaks_the_schema_names_the_segment() {
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"AN","lenght":3}}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(&err, SpecError::SegmentSchema { segment, .. } if segment == "AA"),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("segment \"AA\" does not match the schema: "),
            "{err}"
        );
        assert!(err.to_string().contains("lenght"), "{err}");
    }

    #[test]
    fn every_object_of_the_segment_schema_is_checked_with_its_path() {
        let cases = [
            (r#"[]"#, "segments", "an array"),
            (r#"{"CLP":[]}"#, "segments.CLP", "an array"),
            (
                r#"{"CLP":{"elements":[]}}"#,
                "segments.CLP.elements",
                "an array",
            ),
            (
                r#"{"CLP":{"elements":{"1":"claim_id"}}}"#,
                "segments.CLP.elements.1",
                "a string",
            ),
            (
                r#"{"SVC":{"elements":{"1":{"name":"p","type":"AN","composite":{"2":7}}}}}"#,
                "segments.SVC.elements.1.composite.2",
                "a number",
            ),
        ];
        for (segments, expected_path, expected_found) in cases {
            let json = format!(
                r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{segments}}}"#
            );
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{segments}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patch_retouches_one_element_and_keeps_the_rest() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(r#"{"segments":{"CLP":{"elements":{"1":{"max":30}}}}}"#)
            .unwrap();
        let clp = spec.segment(b"CLP").unwrap();
        assert_eq!(clp.elements[&1].max, Some(30));
        assert_eq!(clp.elements[&1].name, "claim_submitter_id");
        assert_eq!(clp.elements.len(), 3);
    }

    #[test]
    fn a_patch_adds_a_segment_definition() {
        let spec = Spec::from_json(CLP_ONLY)
            .unwrap()
            .merge_patch(
                r#"{"segments":{"ZZ1":{"elements":{"1":{"name":"payer_note","type":"AN"}}}}}"#,
            )
            .unwrap();
        assert_eq!(
            spec.segment(b"ZZ1").unwrap().elements[&1].name,
            "payer_note"
        );
    }

    #[test]
    fn to_json_round_trips_the_segments_section() {
        let spec = Spec::from_json(CLP_ONLY).unwrap();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert!(spec.segments().eq(again.segments()));
        assert_eq!(again.segments().count(), 2);
    }

    #[test]
    fn segment_schema_error_displays_the_segment_and_the_serde_message() {
        let err = SpecError::SegmentSchema {
            segment: "CLP".into(),
            source: serde_json::from_value::<RawSegment>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            err.to_string(),
            "segment \"CLP\" does not match the schema: invalid type: integer `1`, expected a segment object"
        );
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn bad_element_def_displays_segment_position_and_every_reason() {
        let cases = [
            (
                ElementDefError::NonCanonicalPosition,
                "segment \"CLP\" element \"01\": positions are 1-based integers written in canonical form",
            ),
            (
                ElementDefError::EmptyName,
                "segment \"CLP\" element \"01\": \"name\" is empty",
            ),
            (
                ElementDefError::DuplicateName {
                    name: "claim_id".into(),
                    first: "1".into(),
                },
                "segment \"CLP\" element \"01\": name \"claim_id\" is already used by position \"1\"",
            ),
            (
                ElementDefError::UnknownType { found: "XX".into() },
                "segment \"CLP\" element \"01\": type \"XX\" is not one of AN, ID, N0 to N9, R, DT, TM",
            ),
            (
                ElementDefError::ScaleWithoutR { kind: "N2".into() },
                "segment \"CLP\" element \"01\": \"scale\" applies only to type R; found type \"N2\"",
            ),
            (
                ElementDefError::MinAboveMax { min: 5, max: 2 },
                "segment \"CLP\" element \"01\": \"min\" 5 is greater than \"max\" 2",
            ),
            (
                ElementDefError::CompositeOnNonAn { kind: "ID".into() },
                "segment \"CLP\" element \"01\": \"composite\" requires type AN; found type \"ID\"",
            ),
            (
                ElementDefError::NestedComposite,
                "segment \"CLP\" element \"01\": a component cannot declare its own \"composite\"",
            ),
        ];
        for (reason, expected) in cases {
            let err = SpecError::BadElementDef {
                segment: "CLP".into(),
                position: "01".into(),
                reason,
            };
            assert_eq!(err.to_string(), expected);
            assert!(std::error::Error::source(&err).is_none());
        }
    }

    #[test]
    fn an_r_scale_above_18_or_a_zero_max_is_rejected_with_the_value() {
        let cases = [
            (
                r#"{"3":{"name":"a","type":"R","scale":19}}"#,
                ElementDefError::ScaleAboveMaximum { scale: 19 },
            ),
            (
                r#"{"3":{"name":"a","type":"R","scale":200}}"#,
                ElementDefError::ScaleAboveMaximum { scale: 200 },
            ),
            (
                r#"{"3":{"name":"a","type":"AN","max":0}}"#,
                ElementDefError::ZeroMax,
            ),
            (
                r#"{"3":{"name":"a","type":"R","min":0,"max":0}}"#,
                ElementDefError::ZeroMax,
            ),
        ];
        for (elements, expected_reason) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::BadElementDef { segment, position, reason } if segment == "AA" && position == "3" && *reason == expected_reason),
                "{elements}: {err:?}"
            );
        }
        let at_the_cap = Spec::from_json(
            r#"{"name":"t","loops":{"a":{"trigger":{"segment":"AA"}}},"segments":{"AA":{"elements":{"1":{"name":"a","type":"R","scale":18,"max":1}}}}}"#,
        )
        .unwrap();
        assert_eq!(
            at_the_cap.segment(b"AA").unwrap().elements[&1].kind,
            ElementType::R { scale: 18 }
        );
    }

    #[test]
    fn scale_and_max_reasons_display_segment_position_and_value() {
        let at = |reason| SpecError::BadElementDef {
            segment: "CLP".into(),
            position: "12".into(),
            reason,
        };
        assert_eq!(
            at(ElementDefError::ScaleAboveMaximum { scale: 19 }).to_string(),
            "segment \"CLP\" element \"12\": \"scale\" 19 is above the maximum of 18"
        );
        assert_eq!(
            at(ElementDefError::ZeroMax).to_string(),
            "segment \"CLP\" element \"12\": \"max\" is 0; an element holds at least one character"
        );
    }

    const TABLED: &str = r#"{"name":"t",
        "loops":{
            "A":{"trigger":{"segment":"AA"},"segments":["A1"],"end":"AE"},
            "B":{"parent":"A","trigger":{"segment":"BB"},"segments":["B1","AJ"]},
            "C":{"parent":"B","trigger":{"segment":"CC"},"segments":["C1","AJ"]},
            "D":{"parent":"A","trigger":{"segment":"DD"}}
        },
        "segments":{
            "BB":{"elements":{"1":{"name":"id","type":"AN"},"2":{"name":"amount","type":"R"}}},
            "CC":{"elements":{"1":{"name":"code","type":"AN","composite":{
                "1":{"name":"qualifier","type":"ID"},"2":{"name":"value","type":"AN"}}}}}
        },
        "tables":{
            "heads":{"loops":["A"],"ref":"head","columns":{
                "code":{"segment":"AA","element":1},
                "note":{"loop":"D","segment":"DD","where":{"1":"N"},"element":2}
            }},
            "bodies":{"loops":["B"],"ref":"body","columns":{
                "id":{"segment":"BB","element":1},
                "line_at":{"loop":"C","segment":"C1","segment_index":true}
            }},
            "lines":{"loops":["C"],"ref":"line","columns":{
                "code":{"segment":"CC","element":1,"component":2}
            }},
            "adjustments":{"loops":["B","C"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
                "kind":{"element":1},
                "reason":{"group_element":0},
                "amount":{"group_element":1}
            }}
        }
    }"#;

    fn table_error(tables: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{
                "A":{{"trigger":{{"segment":"AA"}}}},
                "B":{{"parent":"A","trigger":{{"segment":"BB"}}}},
                "C":{{"parent":"B","trigger":{{"segment":"CC"}},"segments":["XX"]}},
                "D":{{"parent":"A","trigger":{{"segment":"DD"}},"segments":["XX"]}}
            }},"tables":{tables}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn tables_are_read_in_name_order_with_their_sources() {
        let spec = Spec::from_json(TABLED).unwrap();
        let names: Vec<&str> = spec.tables().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["adjustments", "bodies", "heads", "lines"]);
        let id = |name: &str| spec.loop_id(name).unwrap();
        let heads = spec.table("heads").unwrap();
        assert_eq!(heads.reference, "head");
        assert_eq!(heads.loops, vec![id("A")]);
        assert_eq!((heads.segment.as_ref(), heads.repeat), (None, None));
        assert_eq!(
            heads.columns,
            vec![
                (
                    "code".to_string(),
                    ColumnSource::Element {
                        loop_id: None,
                        segment: b"AA".to_vec(),
                        conditions: Vec::new(),
                        element: 1,
                        component: None,
                    }
                ),
                (
                    "note".to_string(),
                    ColumnSource::Element {
                        loop_id: Some(id("D")),
                        segment: b"DD".to_vec(),
                        conditions: vec![(1, b"N".to_vec())],
                        element: 2,
                        component: None,
                    }
                ),
            ]
        );
        assert_eq!(
            spec.table("bodies").unwrap().columns[1].1,
            ColumnSource::SegmentIndex {
                loop_id: Some(id("C")),
                segment: b"C1".to_vec(),
                conditions: Vec::new(),
            }
        );
        let adjustments = spec.table("adjustments").unwrap();
        assert_eq!(
            adjustments.reference, "adjustments",
            "ref defaults to the name"
        );
        assert_eq!(adjustments.loops, vec![id("B"), id("C")]);
        assert_eq!(adjustments.segment.as_deref(), Some(&b"AJ"[..]));
        assert_eq!(adjustments.repeat, Some(Repeat { from: 2, step: 2 }));
        assert_eq!(
            adjustments.columns,
            vec![
                (
                    "amount".to_string(),
                    ColumnSource::GroupElement {
                        offset: 1,
                        component: None
                    }
                ),
                (
                    "kind".to_string(),
                    ColumnSource::Element {
                        loop_id: None,
                        segment: b"AJ".to_vec(),
                        conditions: Vec::new(),
                        element: 1,
                        component: None,
                    }
                ),
                (
                    "reason".to_string(),
                    ColumnSource::GroupElement {
                        offset: 0,
                        component: None
                    }
                ),
            ]
        );
        assert_eq!(spec.table("missing"), None);
    }

    #[test]
    fn a_table_hangs_from_the_tables_anchored_above_it() {
        let spec = Spec::from_json(TABLED).unwrap();
        let index = |name: &str| spec.tables().iter().position(|t| t.name == name).unwrap();
        let (adjustments, bodies, heads, lines) = (
            index("adjustments"),
            index("bodies"),
            index("heads"),
            index("lines"),
        );
        let table = |i: usize| &spec.tables()[i];
        assert_eq!(
            (table(heads).parent, table(heads).ancestors.clone()),
            (None, vec![])
        );
        assert_eq!(table(bodies).parent, Some(heads));
        assert_eq!(table(lines).ancestors, vec![heads, bodies]);
        assert_eq!(
            table(adjustments).ancestors,
            vec![heads, bodies, lines],
            "a segment table anchored in B and C hangs from the deepest chain"
        );
        assert_eq!(table(adjustments).parent, Some(lines));
    }

    #[test]
    fn element_definitions_are_found_by_segment_position_and_component() {
        let spec = Spec::from_json(TABLED).unwrap();
        assert_eq!(spec.element_def(b"BB", 2, None).unwrap().name, "amount");
        assert_eq!(spec.element_def(b"CC", 1, Some(2)).unwrap().name, "value");
        assert_eq!(spec.element_def(b"CC", 1, Some(3)), None);
        assert_eq!(spec.element_def(b"BB", 9, None), None);
        assert_eq!(spec.element_def(b"ZZ", 1, None), None);
    }

    #[test]
    fn bad_tables_are_rejected_with_the_table_the_column_and_the_reason() {
        let cases: Vec<(&str, &str, Option<&str>, TableDefError)> = vec![
            (
                r#"{"":{"loops":["A"]}}"#,
                "",
                None,
                TableDefError::EmptyName { what: "table name" },
            ),
            (r#"{"t":{"loops":[]}}"#, "t", None, TableDefError::NoLoops),
            (
                r#"{"t":{"loops":["A","B","A"]}}"#,
                "t",
                None,
                TableDefError::DuplicateLoop {
                    loop_name: "A".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":2,"step":2},"columns":{"c":{"group_element":0,"where":{"1":"X"}}}}}"#,
                "t",
                Some("c"),
                TableDefError::AnchorSegmentOnly {
                    key: "where",
                    anchor_segment: "AA".into(),
                    written: r#"{"1":"X"}"#.into(),
                },
            ),
            (
                r#"{"t":{"loops":["Z"]}}"#,
                "t",
                None,
                TableDefError::UnknownLoop { name: "Z".into() },
            ),
            (
                r#"{"t":{"loops":["A"],"ref":""}}"#,
                "t",
                None,
                TableDefError::EmptyName { what: "ref" },
            ),
            (
                r#"{"t":{"loops":["A"],"ref":"segment"}}"#,
                "t",
                None,
                TableDefError::ReservedName {
                    name: "segment".into(),
                },
            ),
            (
                r#"{"a":{"loops":["A"],"ref":"x"},"b":{"loops":["B"],"ref":"x"}}"#,
                "b",
                None,
                TableDefError::RefTaken {
                    name: "x".into(),
                    table: "a".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"repeat":{"from":1,"step":2}}}"#,
                "t",
                None,
                TableDefError::RepeatWithoutSegment,
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":0,"step":2}}}"#,
                "t",
                None,
                TableDefError::ZeroPosition { key: "repeat.from" },
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":1,"step":0}}}"#,
                "t",
                None,
                TableDefError::ZeroStep,
            ),
            (
                r#"{"t":{"loops":["C","A"]}}"#,
                "t",
                None,
                TableDefError::NestedAnchors {
                    outer: "A".into(),
                    inner: "C".into(),
                },
            ),
            (
                r#"{"a":{"loops":["B"]},"b":{"loops":["D","B"]}}"#,
                "b",
                None,
                TableDefError::SharedAnchor {
                    loop_name: "B".into(),
                    other: "a".into(),
                },
            ),
            (
                r#"{"b":{"loops":["B"]},"d":{"loops":["D"]},"x":{"loops":["C","D"],"segment":"XX"}}"#,
                "x",
                None,
                TableDefError::UnrelatedAnchors {
                    first: "C".into(),
                    second: "D".into(),
                    chains: Box::new(AnchorChains {
                        first: vec!["b".into()],
                        second: vec!["d".into()],
                    }),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"":{"segment":"AA","element":1}}}}"#,
                "t",
                Some(""),
                TableDefError::EmptyName {
                    what: "column name",
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"row":{"segment":"AA","element":1}}}}"#,
                "t",
                Some("row"),
                TableDefError::ReservedName { name: "row".into() },
            ),
            (
                r#"{"a":{"loops":["A"],"ref":"head"},"b":{"loops":["B"],"columns":{"head":{"segment":"BB","element":1}}}}"#,
                "b",
                Some("head"),
                TableDefError::ReservedName {
                    name: "head".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA"}}}}"#,
                "t",
                Some("c"),
                TableDefError::SourceCount { found: 0 },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"segment_index":true}}}}"#,
                "t",
                Some("c"),
                TableDefError::SourceCount { found: 2 },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","segment_index":true,"component":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::ComponentOnIndex,
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::ZeroPosition { key: "element" },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"component":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::ZeroPosition { key: "component" },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"group_element":0}}}}"#,
                "t",
                Some("c"),
                TableDefError::GroupWithoutRepeat,
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","repeat":{"from":2,"step":3},"columns":{"c":{"group_element":3}}}}"#,
                "t",
                Some("c"),
                TableDefError::OffsetBeyondStep { offset: 3, step: 3 },
            ),
            (
                r#"{"t":{"loops":["A"],"segment":"AA","columns":{"c":{"loop":"B","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::AnchorSegmentOnly {
                    key: "loop",
                    anchor_segment: "AA".into(),
                    written: r#""B""#.into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NeedsSegment,
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"loop":"Z","segment":"ZZ","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::UnknownLoop { name: "Z".into() },
            ),
            (
                r#"{"t":{"loops":["B","D"],"columns":{"c":{"loop":"C","segment":"CC","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NotADescendant {
                    loop_name: "C".into(),
                    anchor: "D".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"loop":"A","segment":"AA","element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::NotADescendant {
                    loop_name: "A".into(),
                    anchor: "A".into(),
                },
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","where":{"01":"X"},"element":1}}}}"#,
                "t",
                Some("c"),
                TableDefError::BadPosition { key: "01".into() },
            ),
        ];
        for (tables, expected_table, expected_column, expected_reason) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::BadTable { table, column, reason } if table == expected_table && column.as_deref() == expected_column && *reason == expected_reason),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn a_column_or_table_reading_a_segment_its_loop_never_holds_is_rejected() {
        let json = |tables: &str| {
            format!(
                r#"{{"name":"t","loops":{{
                    "A":{{"trigger":{{"segment":"AA"}},"segments":["A1"],"end":"AE"}},
                    "B":{{"parent":"A","trigger":{{"segment":"BB"}}}}
                }},"tables":{tables}}}"#
            )
        };
        let held = json(
            r#"{"t":{"loops":["A"],"columns":{
                "a":{"segment":"AA","element":1},
                "b":{"segment":"A1","element":1},
                "c":{"segment":"AE","element":1}}},
               "u":{"loops":["A"],"segment":"A1","columns":{"x":{"element":1}}}}"#,
        );
        assert!(Spec::from_json(&held).is_ok());
        let bad_column =
            json(r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"REFF","element":1}}}}"#);
        assert_eq!(
            Spec::from_json(&bad_column).unwrap_err().to_string(),
            "table \"t\" column \"c\": segment \"REFF\" is neither the trigger nor a segment of loop \"A\", so it is never read there"
        );
        let bad_loop = json(
            r#"{"t":{"loops":["A"],"columns":{"c":{"loop":"B","segment":"A1","element":1}}}}"#,
        );
        assert!(matches!(
            Spec::from_json(&bad_loop).unwrap_err(),
            SpecError::BadTable {
                reason: TableDefError::SegmentNotHeld { ref loop_name, .. },
                ..
            } if loop_name == "B"
        ));
        let bad_table =
            json(r#"{"t":{"loops":["A"],"segment":"REFF","columns":{"c":{"element":1}}}}"#);
        assert!(matches!(
            Spec::from_json(&bad_table).unwrap_err(),
            SpecError::BadTable {
                column: None,
                reason: TableDefError::SegmentNotHeld { ref segment, ref loop_name },
                ..
            } if segment == "REFF" && loop_name == "A"
        ));
    }

    #[test]
    fn anchor_conflicts_show_the_datum_as_the_spec_writes_it() {
        let err = table_error(
            r#"{"t":{"loops":["A"],"segment":"AA","columns":{"c":{"element":1,"where":{"2":"X"}}}}}"#,
        );
        assert_eq!(
            err.to_string(),
            "table \"t\" column \"c\": \"where\" ({\"2\":\"X\"}) does not apply in a table anchored on segment \"AA\": its columns read that segment"
        );
        let err = table_error(
            r#"{"b":{"loops":["B"]},"d":{"loops":["D"]},"x":{"loops":["C","D"],"segment":"XX"}}"#,
        );
        assert_eq!(
            err.to_string(),
            "table \"x\": anchor loops \"C\" and \"D\" sit under tables that are not one chain: \"C\" under b, \"D\" under d"
        );
    }

    #[test]
    fn empty_segment_ids_in_tables_name_their_key() {
        let cases = [
            (r#"{"t":{"loops":["A"],"segment":""}}"#, "tables.t.segment"),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"","element":1}}}}"#,
                "tables.t.columns.c.segment",
            ),
        ];
        for (tables, expected_key) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::EmptySegmentId { loop_name: None, key } if key == expected_key),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn a_table_that_breaks_the_schema_names_the_table_and_the_column() {
        let err = table_error(r#"{"t":{"loops":["A"],"anchor":"x"}}"#);
        assert!(
            matches!(&err, SpecError::TableSchema { table, column: None, .. } if table == "t"),
            "{err:?}"
        );
        assert!(err.to_string().contains("anchor"), "{err}");
        let err = table_error(r#"{"t":{"loops":["A"],"columns":{"c":{"elemnt":1}}}}"#);
        assert!(
            matches!(&err, SpecError::TableSchema { table, column: Some(column), .. } if table == "t" && column == "c"),
            "{err:?}"
        );
        assert!(err.to_string().contains("elemnt"), "{err}");
    }

    #[test]
    fn every_object_of_the_table_schema_is_checked_with_its_path() {
        let cases = [
            (r#"[]"#, "tables", "an array"),
            (r#"{"t":[]}"#, "tables.t", "an array"),
            (
                r#"{"t":{"loops":["A"],"repeat":[2,3]}}"#,
                "tables.t.repeat",
                "an array",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":[]}}"#,
                "tables.t.columns",
                "an array",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":"CLP01"}}}"#,
                "tables.t.columns.c",
                "a string",
            ),
            (
                r#"{"t":{"loops":["A"],"columns":{"c":{"segment":"AA","element":1,"where":["X"]}}}}"#,
                "tables.t.columns.c.where",
                "an array",
            ),
        ];
        for (tables, expected_path, expected_found) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::NotAnObject { path, found } if path == expected_path && *found == expected_found),
                "{tables}: {err:?}"
            );
        }
    }

    #[test]
    fn a_patch_adds_a_column_with_three_lines() {
        let spec = Spec::from_json(TABLED).unwrap();
        let patched = spec
            .merge_patch(
                r#"{"tables":{"bodies":{"columns":{
                    "amount":{"segment":"BB","element":2}
                }}}}"#,
            )
            .unwrap();
        let names: Vec<&str> = patched
            .table("bodies")
            .unwrap()
            .columns
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["amount", "id", "line_at"]);
    }

    #[test]
    fn to_json_round_trips_the_tables_section() {
        let spec = Spec::from_json(TABLED).unwrap();
        let again = Spec::from_json(&spec.to_json()).unwrap();
        assert_eq!(again.tables(), spec.tables());
        assert_eq!(again.tables().len(), 4);
    }

    #[test]
    fn table_errors_display_the_table_the_column_and_every_reason() {
        let cases = [
            (TableDefError::EmptyName { what: "ref" }, "the ref is empty"),
            (
                TableDefError::NoLoops,
                "\"loops\" is empty; a table anchors in at least one loop",
            ),
            (
                TableDefError::DuplicateLoop {
                    loop_name: "2100".into(),
                },
                "loop \"2100\" is listed more than once in \"loops\"",
            ),
            (
                TableDefError::UnknownLoop {
                    name: "2101".into(),
                },
                "loop \"2101\" does not exist",
            ),
            (
                TableDefError::ReservedName { name: "row".into() },
                "the name \"row\" is taken by an automatic column",
            ),
            (
                TableDefError::RefTaken {
                    name: "claim".into(),
                    table: "claims".into(),
                },
                "\"ref\" \"claim\" is already used by table \"claims\"",
            ),
            (
                TableDefError::RepeatWithoutSegment,
                "\"repeat\" requires \"segment\": only a segment's elements repeat",
            ),
            (TableDefError::ZeroStep, "\"repeat.step\" is 0"),
            (
                TableDefError::ZeroPosition { key: "element" },
                "\"element\" must be a 1-based position; found 0",
            ),
            (
                TableDefError::BadPosition { key: "01".into() },
                "\"where\" position \"01\" is not a 1-based integer in canonical form",
            ),
            (
                TableDefError::OffsetBeyondStep { offset: 3, step: 3 },
                "\"group_element\" 3 is outside a group of 3 elements (offsets start at 0)",
            ),
            (
                TableDefError::NestedAnchors {
                    outer: "2100".into(),
                    inner: "2110".into(),
                },
                "anchor loops \"2100\" and \"2110\" nest; a table without \"segment\" anchors in loops that do not",
            ),
            (
                TableDefError::SharedAnchor {
                    loop_name: "2100".into(),
                    other: "claims".into(),
                },
                "loop \"2100\" already anchors table \"claims\"; a loop anchors at most one table without \"segment\"",
            ),
            (
                TableDefError::UnrelatedAnchors {
                    first: "2110".into(),
                    second: "1000A".into(),
                    chains: Box::new(AnchorChains {
                        first: vec!["payments".into(), "claims".into()],
                        second: vec!["payers".into()],
                    }),
                },
                "anchor loops \"2110\" and \"1000A\" sit under tables that are not one chain: \"2110\" under payments/claims, \"1000A\" under payers",
            ),
            (
                TableDefError::UnrelatedAnchors {
                    first: "2110".into(),
                    second: "1000A".into(),
                    chains: Box::new(AnchorChains {
                        first: vec!["a b".into()],
                        second: Vec::new(),
                    }),
                },
                "anchor loops \"2110\" and \"1000A\" sit under tables that are not one chain: \"2110\" under \"a b\", \"1000A\" under no table",
            ),
            (
                TableDefError::NotADescendant {
                    loop_name: "1000A".into(),
                    anchor: "2100".into(),
                },
                "loop \"1000A\" is not inside anchor loop \"2100\"",
            ),
            (
                TableDefError::SegmentNotHeld {
                    segment: "REFF".into(),
                    loop_name: "2100".into(),
                },
                "segment \"REFF\" is neither the trigger nor a segment of loop \"2100\", so it is never read there",
            ),
            (
                TableDefError::AnchorSegmentOnly {
                    key: "where",
                    anchor_segment: "CLP".into(),
                    written: r#"{"1":"X"}"#.into(),
                },
                "\"where\" ({\"1\":\"X\"}) does not apply in a table anchored on segment \"CLP\": its columns read that segment",
            ),
            (
                TableDefError::AnchorSegmentOnly {
                    key: "segment",
                    anchor_segment: "CLP".into(),
                    written: r#""SVC""#.into(),
                },
                "\"segment\" (\"SVC\") does not apply in a table anchored on segment \"CLP\": its columns read that segment",
            ),
            (
                TableDefError::NeedsSegment,
                "the column names no \"segment\" to read",
            ),
            (
                TableDefError::GroupWithoutRepeat,
                "\"group_element\" requires the table's \"repeat\"",
            ),
            (
                TableDefError::SourceCount { found: 2 },
                "a column takes exactly one of \"element\", \"group_element\" or \"segment_index\"; found 2",
            ),
            (
                TableDefError::ComponentOnIndex,
                "\"component\" does not apply to \"segment_index\"",
            ),
        ];
        for (reason, expected) in cases {
            let in_column = SpecError::BadTable {
                table: "claims".into(),
                column: Some("charge".into()),
                reason: reason.clone(),
            };
            assert_eq!(
                in_column.to_string(),
                format!("table \"claims\" column \"charge\": {expected}")
            );
            let in_table = SpecError::BadTable {
                table: "claims".into(),
                column: None,
                reason,
            };
            assert_eq!(
                in_table.to_string(),
                format!("table \"claims\": {expected}")
            );
            assert!(std::error::Error::source(&in_table).is_none());
        }
    }

    #[test]
    fn table_schema_errors_display_the_table_the_column_and_the_serde_message() {
        let source = || serde_json::from_value::<RawColumn>(serde_json::json!(1)).unwrap_err();
        let in_column = SpecError::TableSchema {
            table: "claims".into(),
            column: Some("charge".into()),
            source: source(),
        };
        assert_eq!(
            in_column.to_string(),
            "table \"claims\" column \"charge\" does not match the schema: invalid type: integer `1`, expected a column object"
        );
        assert!(std::error::Error::source(&in_column).is_some());
        let in_table = SpecError::TableSchema {
            table: "claims".into(),
            column: None,
            source: serde_json::from_value::<RawTable>(serde_json::json!(1)).unwrap_err(),
        };
        assert_eq!(
            in_table.to_string(),
            "table \"claims\" does not match the schema: invalid type: integer `1`, expected a table object"
        );
    }

    fn control_error(control: &str) -> SpecError {
        let json = format!(
            r#"{{"name":"t","loops":{{"env":{{"trigger":{{"segment":"HD"}},"end":"TR","control":{control}}}}}}}"#
        );
        Spec::from_json(&json).unwrap_err()
    }

    #[test]
    fn builtin_835_declares_the_envelope_controls() {
        let spec = Spec::builtin_835();
        let control = |name: &str| spec.get(spec.loop_id(name).unwrap()).control;
        assert_eq!(
            control("interchange"),
            Some(Control {
                opener_element: 13,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Children,
            })
        );
        assert_eq!(
            control("group"),
            Some(Control {
                opener_element: 6,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Children,
            })
        );
        assert_eq!(
            control("transaction"),
            Some(Control {
                opener_element: 2,
                closer_element: 2,
                count_element: 1,
                count: ControlCount::Segments,
            })
        );
        assert_eq!(control("2100"), None);
    }

    #[test]
    fn bad_controls_are_rejected_with_the_loop_and_the_reason() {
        let cases = [
            (
                r#"{"opener_element":0,"closer_element":2,"count_element":1,"count":"segments"}"#,
                ControlError::ZeroPosition {
                    key: "opener_element",
                },
            ),
            (
                r#"{"opener_element":2,"closer_element":2,"count_element":0,"count":"segments"}"#,
                ControlError::ZeroPosition {
                    key: "count_element",
                },
            ),
            (
                r#"{"opener_element":2,"closer_element":2,"count_element":1,"count":"segs"}"#,
                ControlError::UnknownCount {
                    found: "segs".into(),
                },
            ),
        ];
        for (control, expected) in cases {
            let err = control_error(control);
            assert!(
                matches!(&err, SpecError::BadControl { loop_name, reason } if loop_name == "env" && *reason == expected),
                "{control}: {err:?}"
            );
        }
        let err = Spec::from_json(
            r#"{"name":"t","loops":{"env":{"trigger":{"segment":"HD"},"control":{"opener_element":2,"closer_element":2,"count_element":1,"count":"segments"}}}}"#,
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                SpecError::BadControl {
                    reason: ControlError::NoEnd,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn a_control_that_is_not_an_object_or_misses_a_key_is_rejected() {
        let err = control_error("[2,2,1]");
        assert!(
            matches!(&err, SpecError::NotAnObject { path, found: "an array" } if path == "loops.env.control"),
            "{err:?}"
        );
        let err = control_error(r#"{"opener_element":2,"closer_element":2,"count":"segments"}"#);
        assert!(
            matches!(&err, SpecError::Schema { loop_name: Some(name), .. } if name == "env"),
            "{err:?}"
        );
    }

    #[test]
    fn bad_control_displays_the_loop_and_every_reason() {
        let cases = [
            (
                ControlError::ZeroPosition {
                    key: "opener_element",
                },
                "loop \"transaction\" has an invalid \"control\": \"opener_element\" must be a 1-based element position; found 0",
            ),
            (
                ControlError::UnknownCount {
                    found: "segs".into(),
                },
                "loop \"transaction\" has an invalid \"control\": \"count\" must be \"segments\" or \"children\"; found \"segs\"",
            ),
            (
                ControlError::NoEnd,
                "loop \"transaction\" has an invalid \"control\": the loop has no \"end\" segment to check",
            ),
        ];
        for (reason, expected) in cases {
            let err = SpecError::BadControl {
                loop_name: "transaction".into(),
                reason,
            };
            assert_eq!(err.to_string(), expected);
            assert!(std::error::Error::source(&err).is_none());
        }
    }

    #[test]
    fn a_scalar_of_the_wrong_kind_is_rejected_with_its_key_path() {
        let loop_json = |loop_def: &str| format!(r#"{{"name":"t","loops":{{"env":{loop_def}}}}}"#);
        let cases = [
            (
                loop_json(
                    r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":"2","closer_element":2,"count_element":1,"count":"segments"}}"#,
                ),
                "spec: the value at loops.env.control.opener_element must be a non-negative integer; found a string",
            ),
            (
                loop_json(
                    r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":-2,"count_element":1,"count":"segments"}}"#,
                ),
                "spec: the value at loops.env.control.closer_element must be a non-negative integer; found a negative number",
            ),
            (
                loop_json(
                    r#"{"trigger":{"segment":"HD"},"end":"TR","control":{"opener_element":2,"closer_element":2,"count_element":1,"count":7}}"#,
                ),
                "spec: the value at loops.env.control.count must be a string; found a number",
            ),
            (
                loop_json(r#"{"trigger":{"segment":"HD","where":{"1":"X","2":5}}}"#),
                "spec: the value at loops.env.trigger.where.2 must be a string; found a number",
            ),
            (
                loop_json(r#"{"trigger":{"segment":7}}"#),
                "spec: the value at loops.env.trigger.segment must be a string; found a number",
            ),
            (
                loop_json(r#"{"trigger":{"segment":"HD"},"segments":["A",null]}"#),
                "spec: the value at loops.env.segments.1 must be a string; found null",
            ),
            (
                loop_json(r#"{"trigger":{"segment":"HD"},"segments":"A"}"#),
                "spec: the value at loops.env.segments must be an array of strings; found a string",
            ),
            (
                loop_json(r#"{"trigger":{"segment":"HD"},"end":true}"#),
                "spec: the value at loops.env.end must be a string; found a boolean",
            ),
        ];
        for (json, expected) in cases {
            let err = Spec::from_json(&json).unwrap_err();
            assert!(
                matches!(&err, SpecError::WrongType { .. }),
                "{json}: {err:?}"
            );
            assert_eq!(err.to_string(), expected, "{json}");
        }
    }

    #[test]
    fn a_segment_scalar_of_the_wrong_kind_is_rejected_with_its_key_path() {
        let cases = [
            (
                r#"{"3":{"name":"p","type":"AN","min":"1"}}"#,
                "spec: the value at segments.AA.elements.3.min must be a non-negative integer; found a string",
            ),
            (
                r#"{"3":{"name":"p","type":"R","scale":300}}"#,
                "spec: the value at segments.AA.elements.3.scale must be an integer from 0 to 255; found a number above 255",
            ),
            (
                r#"{"3":{"name":"p","type":"AN","required":"yes"}}"#,
                "spec: the value at segments.AA.elements.3.required must be a boolean; found a string",
            ),
            (
                r#"{"3":{"name":5,"type":"AN"}}"#,
                "spec: the value at segments.AA.elements.3.name must be a string; found a number",
            ),
            (
                r#"{"1":{"name":"c","type":"AN","composite":{"2":{"name":"x","type":"AN","max":1.5}}}}"#,
                "spec: the value at segments.AA.elements.1.composite.2.max must be a non-negative integer; found a fractional number",
            ),
        ];
        for (elements, expected) in cases {
            let err = element_error(elements);
            assert!(
                matches!(&err, SpecError::WrongType { .. }),
                "{elements}: {err:?}"
            );
            assert_eq!(err.to_string(), expected, "{elements}");
        }
    }

    #[test]
    fn a_table_scalar_of_the_wrong_kind_is_rejected_with_its_key_path() {
        let cases = [
            (
                r#"{"claims":{"loops":["A",2]}}"#,
                "spec: the value at tables.claims.loops.1 must be a string; found a number",
            ),
            (
                r#"{"claims":{"loops":"A"}}"#,
                "spec: the value at tables.claims.loops must be an array of strings; found a string",
            ),
            (
                r#"{"claims":{"loops":["A"],"repeat":{"from":"2","step":3}}}"#,
                "spec: the value at tables.claims.repeat.from must be a non-negative integer; found a string",
            ),
            (
                r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":"1"}}}}"#,
                "spec: the value at tables.claims.columns.x.element must be a non-negative integer; found a string",
            ),
            (
                r#"{"claims":{"loops":["A"],"columns":{"x":{"segment_index":1}}}}"#,
                "spec: the value at tables.claims.columns.x.segment_index must be a boolean; found a number",
            ),
            (
                r#"{"claims":{"loops":["A"],"columns":{"x":{"segment":"AA","element":1,"where":{"1":2}}}}}"#,
                "spec: the value at tables.claims.columns.x.where.1 must be a string; found a number",
            ),
        ];
        for (tables, expected) in cases {
            let err = table_error(tables);
            assert!(
                matches!(&err, SpecError::WrongType { .. }),
                "{tables}: {err:?}"
            );
            assert_eq!(err.to_string(), expected, "{tables}");
        }
    }

    #[test]
    fn wrong_type_displays_the_path_what_is_required_and_what_was_found() {
        let err = SpecError::WrongType {
            path: "loops.env.control.opener_element".into(),
            expected: "a non-negative integer",
            found: "a string",
        };
        assert_eq!(
            err.to_string(),
            "spec: the value at loops.env.control.opener_element must be a non-negative integer; found a string"
        );
    }

    #[test]
    fn patch_adds_a_loop() {
        let spec = Spec::builtin_835().merge_patch(
            r#"{"loops":{"2100-ZZ":{"parent":"2100","trigger":{"segment":"ZZ1"},"segments":["ZZ2"]}}}"#,
        )
        .unwrap();
        assert_eq!(spec.loops().len(), 9);
        let zz = spec.loop_id("2100-ZZ").unwrap();
        assert_eq!(spec.get(zz).parent, spec.loop_id("2100"));
        assert!(spec.children(spec.loop_id("2100")).contains(&zz));
    }

    #[test]
    fn patch_replaces_arrays_wholesale() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"1000A":{"segments":["N3"]}}}"#)
            .unwrap();
        assert_eq!(
            spec.get(spec.loop_id("1000A").unwrap()).segments,
            vec![b"N3".to_vec()]
        );
    }

    #[test]
    fn patch_merges_nested_objects_and_keeps_siblings() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"1000A":{"trigger":{"where":{"2":"ACME"}}}}}"#)
            .unwrap();
        let trigger = &spec.get(spec.loop_id("1000A").unwrap()).trigger;
        assert_eq!(trigger.segment, b"N1");
        assert_eq!(
            trigger.conditions,
            vec![(1, b"PR".to_vec()), (2, b"ACME".to_vec())]
        );
        assert_eq!(spec.loops().len(), 8, "other loops untouched");
    }

    #[test]
    fn patch_null_deletes() {
        let spec = Spec::builtin_835()
            .merge_patch(
                r#"{"loops":{"2110":null},"tables":{"services":null,"adjustments":{"loops":["2100"]}}}"#,
            )
            .unwrap();
        assert_eq!(spec.loop_id("2110"), None);
        assert!(spec.children(spec.loop_id("2100")).is_empty());
        assert_eq!(spec.table("services"), None);
    }

    #[test]
    fn deleting_a_loop_a_table_anchors_in_names_the_table() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2110":null}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: table \"adjustments\": loop \"2110\" does not exist"
        );
    }

    #[test]
    fn patches_chain_and_to_json_shows_them() {
        let spec = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"a":{"trigger":{"segment":"AA"}}}}"#)
            .unwrap()
            .merge_patch(r#"{"loops":{"b":{"trigger":{"segment":"BB"}}}}"#)
            .unwrap();
        assert_eq!(spec.loops().len(), 10);
        assert!(spec.to_json().contains("\"AA\"") && spec.to_json().contains("\"BB\""));
    }

    #[test]
    fn invalid_patch_result_is_rejected_like_any_spec() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2100":{"parent":"nope"}}}"#)
            .unwrap_err();
        assert!(
            matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::UnknownParent { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn an_unparsable_patch_is_reported_as_a_patch_error() {
        let err = Spec::builtin_835().merge_patch("not json").unwrap_err();
        assert!(
            matches!(&err, SpecError::Patch { source } if matches!(**source, SpecError::Json(_))),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("applying patch: invalid spec JSON: "),
            "{err}"
        );
    }

    #[test]
    fn a_patch_that_breaks_a_reference_says_it_came_from_the_patch() {
        let err = Spec::builtin_835()
            .merge_patch(r#"{"loops":{"2000":null}}"#)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "applying patch: loop \"2100\" names unknown parent \"2000\""
        );
        let source = std::error::Error::source(&err).map(ToString::to_string);
        assert_eq!(
            source.as_deref(),
            Some("loop \"2100\" names unknown parent \"2000\"")
        );
    }

    #[test]
    fn merge_patch_follows_rfc_7386() {
        use serde_json::json;
        let cases = [
            (json!({"a": "b"}), json!({"a": "c"}), json!({"a": "c"})),
            (
                json!({"a": "b"}),
                json!({"b": "c"}),
                json!({"a": "b", "b": "c"}),
            ),
            (json!({"a": "b"}), json!({"a": null}), json!({})),
            (
                json!({"a": "b", "b": "c"}),
                json!({"a": null}),
                json!({"b": "c"}),
            ),
            (json!({"a": ["b"]}), json!({"a": "c"}), json!({"a": "c"})),
            (json!({"a": "c"}), json!({"a": ["b"]}), json!({"a": ["b"]})),
            (
                json!({"a": {"b": "c"}}),
                json!({"a": {"b": "d", "c": null}}),
                json!({"a": {"b": "d"}}),
            ),
            (
                json!({"a": [{"b": "c"}]}),
                json!({"a": [1]}),
                json!({"a": [1]}),
            ),
            (json!(["a", "b"]), json!(["c", "d"]), json!(["c", "d"])),
            (json!({"a": "b"}), json!(["c"]), json!(["c"])),
            (json!({"a": "foo"}), json!(null), json!(null)),
            (json!({"a": "foo"}), json!("bar"), json!("bar")),
            (
                json!({"e": null}),
                json!({"a": 1}),
                json!({"e": null, "a": 1}),
            ),
            (
                json!([1, 2]),
                json!({"a": "b", "c": null}),
                json!({"a": "b"}),
            ),
            (
                json!({}),
                json!({"a": {"bb": {"ccc": null}}}),
                json!({"a": {"bb": {}}}),
            ),
        ];
        for (target, patch, expected) in cases {
            let mut result = target.clone();
            merge_patch(&mut result, &patch);
            assert_eq!(result, expected, "target {target} patch {patch}");
        }
    }
}
