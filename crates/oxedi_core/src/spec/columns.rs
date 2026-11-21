//! Compilation of one table column into its value source.

use super::Spec;
use super::error::SpecError;
use super::loops::LoopId;
use super::occurrences::OccurrenceDef;
use super::raw::{RawColumn, RawPick};
use super::segments::{ROW_COLUMN, SEGMENT_COLUMN, parse_position};
use super::table_error::{AnchorPositions, LoopSegments, TableDefError};
use super::tables::{ColumnSource, Pick, Repeat};
use crate::message::render_key;

/// One refusal of a column's keys: the error when `raw`, written for column
/// `column`, breaks it.
type ColumnRule = fn(&str, &RawColumn) -> Option<TableDefError>;

/// The refusals of the column's name, checked before its source keys are read.
const NAME_RULES: [ColumnRule; 2] = [empty_name, reserved_name];

/// The refusals of a position key, checked once the column is known to write
/// one source key.
const KEY_RULES: [ColumnRule; 3] = [component_on_index, zero_element, zero_component];

fn empty_name(column: &str, _: &RawColumn) -> Option<TableDefError> {
    column.is_empty().then_some(TableDefError::EmptyName {
        what: "column name",
    })
}

fn reserved_name(column: &str, _: &RawColumn) -> Option<TableDefError> {
    [ROW_COLUMN, SEGMENT_COLUMN]
        .contains(&column)
        .then(|| TableDefError::ReservedName {
            name: column.to_string(),
        })
}

fn component_on_index(_: &str, raw: &RawColumn) -> Option<TableDefError> {
    (raw.segment_index && raw.component.is_some()).then_some(TableDefError::ComponentOnIndex)
}

fn zero_element(_: &str, raw: &RawColumn) -> Option<TableDefError> {
    (raw.element == Some(0)).then_some(TableDefError::ZeroPosition { key: "element" })
}

fn zero_component(_: &str, raw: &RawColumn) -> Option<TableDefError> {
    (raw.component == Some(0)).then_some(TableDefError::ZeroPosition { key: "component" })
}

/// The first rule of `rules` that `raw` breaks, as an error.
fn refuse(rules: &[ColumnRule], column: &str, raw: &RawColumn) -> Result<(), TableDefError> {
    rules
        .iter()
        .find_map(|rule| rule(column, raw))
        .map_or(Ok(()), Err)
}

/// The one source key a column writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnRead {
    /// `element`: a 1-based element position.
    Element(usize),
    /// `group_element`: a 0-based position inside the repeated group.
    Group(usize),
    /// `segment_index`: the index of the segment read.
    Index,
}

impl ColumnRead {
    /// The source key `raw` writes; `SourceCount` unless it writes exactly one.
    fn read(raw: &RawColumn) -> Result<Self, TableDefError> {
        match (raw.element, raw.group_element, raw.segment_index) {
            (Some(element), None, false) => Ok(Self::Element(element)),
            (None, Some(offset), false) => Ok(Self::Group(offset)),
            (None, None, true) => Ok(Self::Index),
            _ => Err(TableDefError::SourceCount {
                found: usize::from(raw.element.is_some())
                    + usize::from(raw.group_element.is_some())
                    + usize::from(raw.segment_index),
            }),
        }
    }
}

/// The refusals of a column's keys that need nothing from the spec, in the
/// order a spec reports them, and the source key it writes.
fn check_keys(
    column: &str,
    raw: &RawColumn,
    anchor_segment: Option<&[u8]>,
) -> Result<ColumnRead, TableDefError> {
    refuse(&NAME_RULES, column, raw)?;
    let read = ColumnRead::read(raw)?;
    refuse(&KEY_RULES, column, raw)?;
    anchor_only(raw, anchor_segment)?;
    Ok(read)
}

/// The keys that place a column's segment, in the order refusals name them,
/// each with its value as the spec writes it (`None` when not written).
fn location_keys(raw: &RawColumn) -> [(&'static str, Option<String>); 5] {
    [
        ("segment", raw.segment.as_ref().map(json)),
        ("loop", raw.loop_name.as_ref().map(json)),
        (
            "where",
            (!raw.conditions.is_empty()).then(|| json(&raw.conditions)),
        ),
        ("occurrence", raw.occurrence.as_ref().map(json)),
        ("pick", raw.pick.as_ref().map(RawPick::written)),
    ]
}

/// The first written key of `keys`, with its value.
fn first_written(
    keys: impl IntoIterator<Item = (&'static str, Option<String>)>,
) -> Option<(&'static str, String)> {
    keys.into_iter()
        .find_map(|(key, value)| value.map(|written| (key, written)))
}

/// Refuses a location key in a table anchored on a segment: its columns read
/// that segment.
fn anchor_only(raw: &RawColumn, anchor_segment: Option<&[u8]>) -> Result<(), TableDefError> {
    let Some(anchor) = anchor_segment else {
        return Ok(());
    };
    first_written(location_keys(raw)).map_or(Ok(()), |(key, written)| {
        Err(TableDefError::AnchorSegmentOnly {
            key,
            anchor_segment: String::from_utf8_lossy(anchor).into_owned(),
            written,
        })
    })
}

/// A `group_element` source, checked against the table's repeat.
fn group_element(
    offset: usize,
    component: Option<usize>,
    repeat: Option<Repeat>,
) -> Result<ColumnSource, TableDefError> {
    let Some(repeat) = repeat else {
        return Err(TableDefError::GroupWithoutRepeat);
    };
    if offset >= repeat.step {
        return Err(TableDefError::OffsetBeyondStep {
            offset,
            step: repeat.step,
        });
    }
    Ok(ColumnSource::GroupElement { offset, component })
}

/// The error of column `column` of table `table`.
fn bad_column(table: &str, column: &str, reason: TableDefError) -> SpecError {
    SpecError::BadTable {
        table: table.to_string(),
        column: Some(column.to_string()),
        reason,
    }
}

/// Compiles the column `column` of table `table`, anchored in `anchors` (on
/// `anchor_segment`, when the table has one).
pub(super) fn compile_column(
    spec: &Spec,
    table: &str,
    column: &str,
    raw: &RawColumn,
    anchors: &[LoopId],
    anchor_segment: Option<&[u8]>,
    repeat: Option<Repeat>,
) -> Result<ColumnSource, SpecError> {
    let fail = |reason| bad_column(table, column, reason);
    match check_keys(column, raw, anchor_segment).map_err(fail)? {
        ColumnRead::Group(offset) => group_element(offset, raw.component, repeat).map_err(fail),
        ColumnRead::Element(element) => {
            Ok(locate(spec, table, column, raw, anchors, anchor_segment)?
                .element(element, raw.component))
        }
        ColumnRead::Index => Ok(locate(spec, table, column, raw, anchors, anchor_segment)?.index()),
    }
}

/// Where a column of an element or index source reads: the anchor segment, or
/// the segment or occurrence it names in its reading loop, checked to arrive
/// before the rows are appended.
fn locate(
    spec: &Spec,
    table: &str,
    column: &str,
    raw: &RawColumn,
    anchors: &[LoopId],
    anchor_segment: Option<&[u8]>,
) -> Result<Read, SpecError> {
    if let Some(anchor) = anchor_segment {
        return Ok(Read::anchored(anchor));
    }
    let fail = |reason| bad_column(table, column, reason);
    let loop_id = reading_loop(spec, raw, anchors).map_err(fail)?;
    let read = match &raw.occurrence {
        Some(name) => by_occurrence(spec, raw, name, loop_id, anchors).map_err(fail)?,
        None => by_segment(spec, table, column, raw, loop_id, anchors)?,
    };
    check_reached(spec, anchors, &read).map_err(fail)?;
    Ok(read)
}

/// What a column of a table without `segment` reads, before its value.
struct Read {
    loop_id: Option<LoopId>,
    segment: Vec<u8>,
    conditions: Vec<(usize, Vec<u8>)>,
    occurrence: Option<String>,
    pick: Pick,
}

impl Read {
    /// The anchor segment itself, read in the anchor loop.
    fn anchored(segment: &[u8]) -> Self {
        Self {
            loop_id: None,
            segment: segment.to_vec(),
            conditions: Vec::new(),
            occurrence: None,
            pick: Pick::First,
        }
    }

    /// The element source reading `element` (and `component`) of this segment.
    fn element(self, element: usize, component: Option<usize>) -> ColumnSource {
        ColumnSource::Element {
            loop_id: self.loop_id,
            segment: self.segment,
            conditions: self.conditions,
            occurrence: self.occurrence,
            pick: self.pick,
            element,
            component,
        }
    }

    /// The index source of this segment.
    fn index(self) -> ColumnSource {
        ColumnSource::SegmentIndex {
            loop_id: self.loop_id,
            segment: self.segment,
            conditions: self.conditions,
            occurrence: self.occurrence,
            pick: self.pick,
        }
    }
}

/// A value as JSON, for an error that shows what the spec wrote.
fn json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The column's `loop`: `None` when it names none or names the anchor loop
/// itself; otherwise a loop in line with each anchor loop. Each anchor is
/// checked on its own here; that the loop is then inside every anchor or
/// above every one follows from anchors never nesting (`NestedAnchors`) and
/// from this check rejecting a loop out of line with any anchor
/// (`UnrelatedLoop`).
fn reading_loop(
    spec: &Spec,
    raw: &RawColumn,
    anchors: &[LoopId],
) -> Result<Option<LoopId>, TableDefError> {
    let Some(name) = &raw.loop_name else {
        return Ok(None);
    };
    let id = spec
        .loop_id(name)
        .ok_or_else(|| TableDefError::UnknownLoop { name: name.clone() })?;
    let in_line = |anchor: LoopId| {
        id == anchor || spec.ancestors(id).contains(&anchor) || spec.ancestors(anchor).contains(&id)
    };
    if let Some(&anchor) = anchors.iter().find(|&&anchor| !in_line(anchor)) {
        return Err(TableDefError::UnrelatedLoop {
            loop_name: name.clone(),
            anchor: spec.loop_name(anchor).to_string(),
        });
    }
    Ok((!anchors.contains(&id)).then_some(id))
}

/// The occurrence a loop's trigger opens on, if the loop declares one.
fn trigger_occurrence(spec: &Spec, id: LoopId) -> Option<&OccurrenceDef> {
    let def = spec.get(id);
    def.trigger_occurrence()
        .and_then(|index| def.occurrences.get(index))
}

/// Rejects a source in loop `id` above an anchor whose segments all come
/// after the position where the anchor opens: they arrive once the anchor's
/// rows are appended, so the column would never fill. Positions compare
/// only when every loop from `id` down to the anchor shares one position
/// space (each trigger occurrence after its parent's).
fn check_reached(spec: &Spec, anchors: &[LoopId], read: &Read) -> Result<(), TableDefError> {
    let Some(id) = read.loop_id else {
        return Ok(());
    };
    let def = spec.get(id);
    let candidates: Vec<&OccurrenceDef> = match &read.occurrence {
        Some(name) => def.occurrences.iter().filter(|o| o.name == *name).collect(),
        None => def
            .occurrences
            .iter()
            .filter(|o| o.segment == read.segment)
            .collect(),
    };
    let Some(earliest) = candidates.into_iter().min_by_key(|o| o.pos) else {
        return Ok(());
    };
    for &anchor in anchors {
        if !spec.ancestors(anchor).contains(&id) {
            continue;
        }
        let Some(anchor_pos) = shared_trigger_pos(spec, id, anchor) else {
            continue;
        };
        if earliest.pos > anchor_pos {
            return Err(TableDefError::AfterAnchor {
                occurrence: earliest.name.clone(),
                positions: Box::new(AnchorPositions {
                    loop_name: def.name.clone(),
                    pos: earliest.pos,
                    anchor: spec.loop_name(anchor).to_string(),
                    anchor_pos,
                }),
            });
        }
    }
    Ok(())
}

/// The position of `anchor`'s trigger occurrence when every loop from
/// `above` down to `anchor` shares one position space; `None` otherwise.
fn shared_trigger_pos(spec: &Spec, above: LoopId, anchor: LoopId) -> Option<usize> {
    let anchor_pos = trigger_occurrence(spec, anchor)?.pos;
    let mut child = anchor;
    let mut child_pos = anchor_pos;
    while child != above {
        let parent = spec.get(child).parent?;
        let parent_pos = trigger_occurrence(spec, parent)?.pos;
        if child_pos <= parent_pos {
            return None;
        }
        child = parent;
        child_pos = parent_pos;
    }
    Some(anchor_pos)
}

/// The pick as compiled: `first`, `last` or a position from 1.
fn compile_pick(raw: Option<&RawPick>) -> Result<Pick, TableDefError> {
    raw.map_or(Ok(Pick::default()), |raw| {
        Pick::read(raw).map_err(|written| TableDefError::BadPick { written })
    })
}

/// A source that names the occurrence `name` of the reading loops: `loop_id`,
/// or every anchor loop.
fn by_occurrence(
    spec: &Spec,
    raw: &RawColumn,
    name: &str,
    loop_id: Option<LoopId>,
    anchors: &[LoopId],
) -> Result<Read, TableDefError> {
    if let Some(segment) = &raw.segment {
        return Err(TableDefError::OccurrenceAndSegment {
            key: "segment",
            written: json(segment),
        });
    }
    if !raw.conditions.is_empty() {
        return Err(TableDefError::OccurrenceAndSegment {
            key: "where",
            written: json(&raw.conditions),
        });
    }
    let pick = compile_pick(raw.pick.as_ref())?;
    let readers = loop_id.map_or_else(|| anchors.to_vec(), |id| vec![id]);
    let mut first: Option<(LoopId, &[u8])> = None;
    for reader in readers {
        let def = spec.get(reader);
        let Some(found) = def.occurrences.iter().find(|o| o.name == name) else {
            let mut known: Box<[String]> = def.occurrences.iter().map(|o| o.name.clone()).collect();
            known.sort();
            return Err(TableDefError::UnknownOccurrence {
                loop_name: def.name.clone(),
                occurrence: name.to_string(),
                known,
            });
        };
        match first {
            None => first = Some((reader, &found.segment)),
            Some((loop_one, segment)) if segment != found.segment.as_slice() => {
                return Err(TableDefError::OccurrenceSegments {
                    occurrence: name.to_string(),
                    segments: Box::new(LoopSegments {
                        first_loop: spec.loop_name(loop_one).to_string(),
                        first_segment: String::from_utf8_lossy(segment).into_owned(),
                        loop_name: def.name.clone(),
                        segment: String::from_utf8_lossy(&found.segment).into_owned(),
                    }),
                });
            }
            Some(_) => {}
        }
        if raw.pick.is_some() {
            match (pick, found.max) {
                (_, Some(1)) => {
                    return Err(TableDefError::PickOnSingle {
                        loop_name: def.name.clone(),
                        occurrence: name.to_string(),
                        pick,
                    });
                }
                (Pick::Nth(nth), Some(max)) if nth > max => {
                    return Err(TableDefError::PickBeyondMax {
                        nth,
                        loop_name: def.name.clone(),
                        occurrence: name.to_string(),
                        max,
                    });
                }
                _ => {}
            }
        }
    }
    Ok(Read {
        loop_id,
        segment: first
            .map(|(_, segment)| segment.to_vec())
            .unwrap_or_default(),
        conditions: Vec::new(),
        occurrence: Some(name.to_string()),
        pick,
    })
}

/// A source that names a segment and the values some of its elements hold.
fn by_segment(
    spec: &Spec,
    table: &str,
    column: &str,
    raw: &RawColumn,
    loop_id: Option<LoopId>,
    anchors: &[LoopId],
) -> Result<Read, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: table.to_string(),
        column: Some(column.to_string()),
        reason,
    };
    if let Some(pick) = &raw.pick {
        return Err(fail(TableDefError::PickNeedsOccurrence {
            written: pick.written(),
        }));
    }
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
    let mut conditions = Vec::with_capacity(raw.conditions.len());
    for (key, value) in &raw.conditions {
        let position = parse_position(key)
            .ok_or_else(|| fail(TableDefError::BadPosition { key: key.clone() }))?;
        conditions.push((position, value.as_bytes().to_vec()));
    }
    conditions.sort();
    let readers = loop_id.map_or_else(|| anchors.to_vec(), |id| vec![id]);
    check_held(spec, &readers, &segment).map_err(fail)?;
    Ok(Read {
        loop_id,
        segment,
        conditions,
        occurrence: None,
        pick: Pick::First,
    })
}

/// Fails with the first loop of `readers` that neither triggers on `segment`
/// nor holds it.
pub(super) fn check_held(
    spec: &Spec,
    readers: &[LoopId],
    segment: &[u8],
) -> Result<(), TableDefError> {
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
