//! Reading of the occurrence a table column names: the occurrence in each
//! reading loop, its segment and the pick written for it.

use super::Spec;
use super::columns::{Read, first_written, location_keys};
use super::loops::LoopId;
use super::occurrences::OccurrenceDef;
use super::raw::{RawColumn, RawPick};
use super::table_error::{LoopSegments, TableDefError};
use super::tables::Pick;

/// The pick as compiled: `first`, `last` or a position from 1.
fn compile_pick(raw: Option<&RawPick>) -> Result<Pick, TableDefError> {
    raw.map_or(Ok(Pick::default()), |raw| {
        Pick::read(raw).map_err(|written| TableDefError::BadPick { written })
    })
}

/// The keys an occurrence already names, which a column naming one must not
/// write.
const NAMED_BY_OCCURRENCE: [&str; 2] = ["segment", "where"];

/// Refuses `segment` or `where` next to `occurrence`.
fn occurrence_alone(raw: &RawColumn) -> Result<(), TableDefError> {
    let named = location_keys(raw)
        .into_iter()
        .filter(|(key, _)| NAMED_BY_OCCURRENCE.contains(key));
    first_written(named).map_or(Ok(()), |(key, written)| {
        Err(TableDefError::OccurrenceAndSegment { key, written })
    })
}

/// The occurrence a column names, as one reading loop declares it.
struct LoopOccurrence<'a> {
    /// The reading loop's name.
    loop_name: &'a str,
    /// Its occurrence of that name.
    occurrence: &'a OccurrenceDef,
}

impl<'a> LoopOccurrence<'a> {
    /// Occurrence `name` of loop `reader`; `UnknownOccurrence` with the sorted
    /// names it declares.
    fn find(spec: &'a Spec, reader: LoopId, name: &str) -> Result<Self, TableDefError> {
        let def = spec.get(reader);
        let Some(occurrence) = def.occurrences.iter().find(|o| o.name == name) else {
            let mut known: Box<[String]> = def.occurrences.iter().map(|o| o.name.clone()).collect();
            known.sort();
            return Err(TableDefError::UnknownOccurrence {
                loop_name: def.name.clone(),
                occurrence: name.to_string(),
                known,
            });
        };
        Ok(Self {
            loop_name: &def.name,
            occurrence,
        })
    }
}

/// Refuses an occurrence whose segment differs from the one the first reading
/// loop gives it.
fn same_segment(
    first: &LoopOccurrence<'_>,
    found: &LoopOccurrence<'_>,
) -> Result<(), TableDefError> {
    let (first_segment, segment) = (&first.occurrence.segment, &found.occurrence.segment);
    if first_segment == segment {
        return Ok(());
    }
    Err(TableDefError::OccurrenceSegments {
        occurrence: found.occurrence.name.clone(),
        segments: Box::new(LoopSegments {
            first_loop: first.loop_name.to_string(),
            first_segment: String::from_utf8_lossy(first_segment).into_owned(),
            loop_name: found.loop_name.to_string(),
            segment: String::from_utf8_lossy(segment).into_owned(),
        }),
    })
}

/// One refusal of a written pick for an occurrence.
type PickRule = fn(Pick, &LoopOccurrence<'_>) -> Option<TableDefError>;

/// Checked in order: a single occurrence refuses any pick before a position
/// past a larger max is looked at.
const PICK_RULES: [PickRule; 2] = [pick_on_single, pick_beyond_max];

fn pick_on_single(pick: Pick, found: &LoopOccurrence<'_>) -> Option<TableDefError> {
    (found.occurrence.max == Some(1)).then(|| TableDefError::PickOnSingle {
        loop_name: found.loop_name.to_string(),
        occurrence: found.occurrence.name.clone(),
        pick,
    })
}

fn pick_beyond_max(pick: Pick, found: &LoopOccurrence<'_>) -> Option<TableDefError> {
    match (pick, found.occurrence.max) {
        (Pick::Nth(nth), Some(max)) if nth > max => Some(TableDefError::PickBeyondMax {
            nth,
            loop_name: found.loop_name.to_string(),
            occurrence: found.occurrence.name.clone(),
            max,
        }),
        _ => None,
    }
}

/// The first rule of `PICK_RULES` that the written pick breaks on `found`.
fn check_pick(found: &LoopOccurrence<'_>, written: Option<Pick>) -> Result<(), TableDefError> {
    written.map_or(Ok(()), |pick| {
        PICK_RULES
            .iter()
            .find_map(|rule| rule(pick, found))
            .map_or(Ok(()), Err)
    })
}

/// A later reading loop's occurrence, checked against the first loop's and
/// the written pick.
fn check_reader(
    spec: &Spec,
    reader: LoopId,
    name: &str,
    first: &LoopOccurrence<'_>,
    written: Option<Pick>,
) -> Result<(), TableDefError> {
    let found = LoopOccurrence::find(spec, reader, name)?;
    same_segment(first, &found)?;
    check_pick(&found, written)
}

/// A source that names the occurrence `name` of the reading loops: `loop_id`,
/// or every anchor loop.
pub(super) fn by_occurrence(
    spec: &Spec,
    raw: &RawColumn,
    name: &str,
    loop_id: Option<LoopId>,
    anchors: &[LoopId],
) -> Result<Read, TableDefError> {
    occurrence_alone(raw)?;
    let pick = compile_pick(raw.pick.as_ref())?;
    let written = raw.pick.is_some().then_some(pick);
    let readers = loop_id.as_ref().map_or(anchors, std::slice::from_ref);
    let first = readers
        .first()
        .map(|&reader| LoopOccurrence::find(spec, reader, name))
        .transpose()?;
    if let Some(first) = &first {
        check_pick(first, written)?;
        readers
            .iter()
            .skip(1)
            .try_for_each(|&reader| check_reader(spec, reader, name, first, written))?;
    }
    Ok(Read {
        loop_id,
        segment: first.map_or_else(Vec::new, |first| first.occurrence.segment.clone()),
        conditions: Vec::new(),
        occurrence: Some(name.to_string()),
        pick,
    })
}
