//! The places the projector reads each column from, against the places the
//! write plan writes it to, over the built-in specs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::spec::test_support::{builtin, segs};
use crate::spec::{
    ColumnSource, LoopId, OccurrencePlace, Qualifier, Reading as SpecReading, Spec, TableDef,
};
use crate::spec_load::{Builtin, Version};
use crate::validate::{Validation, validate};
use crate::vocabulary::Pick;
use crate::write::codes::landing;
use crate::write::{SegmentSource, ValueSource, WritePlan};

/// The `edi_835_parser` patch package, whose spec the writer refuses as a whole.
const COMPAT: &str = include_str!("../../../specs/edi_835_parser.json");

/// What of a segment a place reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum At {
    /// An element, or one component of it.
    Element {
        element: usize,
        component: Option<usize>,
    },
    /// The element at 0-based `offset` inside each group of a repeating segment.
    Group {
        offset: usize,
        component: Option<usize>,
    },
}

/// One place a column is read from or written to, by name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    table: String,
    column: String,
    loop_name: String,
    occurrence: String,
    at: At,
    /// The repeat (from 1) a column of a loop table takes; `None` in a table
    /// anchored on a segment, which reads and writes every such segment.
    nth: Option<usize>,
}

/// One column read in one loop: the occurrences the reader takes its
/// segments from, what of them it reads (`None` for a segment's index,
/// which the file holds no element for) and the repeat.
struct Reading {
    table: usize,
    column: usize,
    loop_id: LoopId,
    occurrences: BTreeSet<usize>,
    /// The compiled place the writer lands the column by.
    place: OccurrencePlace,
    at: Option<At>,
    nth: Option<usize>,
}

/// The repeat the writer gives a pick; a `last` alone in its occurrence is
/// the first it writes.
fn repeat(pick: Pick) -> usize {
    match pick {
        Pick::First | Pick::Last => 1,
        Pick::Nth(nth) => nth,
    }
}

/// A segment `id` that holds `conditions` and, at a place they leave free,
/// the qualifier's first code.
fn probe(id: &[u8], conditions: &[(usize, Vec<u8>)], qualifier: Option<&Qualifier>) -> Vec<u8> {
    let mut elements: BTreeMap<usize, Vec<u8>> = conditions.iter().cloned().collect();
    if let Some(qualifier) = qualifier
        && let Some(code) = qualifier.codes.first()
        && !elements.contains_key(&qualifier.element)
    {
        let lead = vec![b':'; qualifier.component.map_or(0, |component| component - 1)];
        let value = [lead.as_slice(), code.as_bytes()].concat();
        elements.insert(qualifier.element, value);
    }
    let last = elements.keys().next_back().copied().unwrap_or(0);
    let mut bytes = id.to_vec();
    for position in 1..=last {
        bytes.push(b'*');
        if let Some(value) = elements.get(&position) {
            bytes.extend_from_slice(value);
        }
    }
    bytes.push(b'~');
    bytes
}

/// The occurrences the engine gives a segment of `segment` in `loop_id` that
/// holds `conditions`: one probe per occurrence of that id, so each
/// occurrence a matching segment can take shows up; a segment no occurrence
/// takes gives none.
fn engine_occurrences(
    spec: &Spec,
    loop_id: LoopId,
    segment: &[u8],
    conditions: &[(usize, Vec<u8>)],
) -> BTreeSet<usize> {
    let mut found = BTreeSet::new();
    for occurrence in &spec.get(loop_id).occurrences {
        if occurrence.segment != segment {
            continue;
        }
        let bytes = probe(segment, conditions, occurrence.qualifier.as_ref());
        if let Some(parsed) = segs(&bytes).first()
            && parsed.holds(conditions)
            && let Some(id) = spec.occurrence_of(loop_id, parsed)
        {
            found.insert(id.index());
        }
    }
    found
}

/// The segment and conditions a column reads, the occurrence it names, and
/// what of the segment.
type Target<'s> = (
    &'s [u8],
    &'s [(usize, Vec<u8>)],
    Option<&'s str>,
    Option<At>,
);

fn target<'s>(def: &'s TableDef, source: &'s ColumnSource) -> Target<'s> {
    match source {
        ColumnSource::Element {
            segment,
            conditions,
            occurrence,
            element,
            component,
            ..
        } => {
            let at = At::Element {
                element: *element,
                component: *component,
            };
            (segment, conditions, occurrence.as_deref(), Some(at))
        }
        ColumnSource::SegmentIndex {
            segment,
            conditions,
            occurrence,
            ..
        } => (segment, conditions, occurrence.as_deref(), None),
        ColumnSource::GroupElement { offset, component } => {
            let at = At::Group {
                offset: *offset,
                component: *component,
            };
            (
                def.segment.as_deref().unwrap_or_default(),
                &[],
                None,
                Some(at),
            )
        }
    }
}

/// The pick of a column's source; a group column takes none.
fn pick_of(source: &ColumnSource) -> Option<Pick> {
    match source {
        ColumnSource::Element { pick, .. } | ColumnSource::SegmentIndex { pick, .. } => Some(*pick),
        ColumnSource::GroupElement { .. } => None,
    }
}

/// Every column the projector reads, with the occurrences it reads it in,
/// from the places the spec compiled.
fn readings(spec: &Spec) -> Vec<Reading> {
    let mut readings = Vec::new();
    for (table, def) in spec.tables().iter().enumerate() {
        for (column, place) in spec.places(table).iter().enumerate() {
            let (_, source) = &def.columns[column];
            let (segment, conditions, _, at) = target(def, source);
            let reads: Vec<(LoopId, Option<usize>, Option<usize>, OccurrencePlace)> = match &place
                .reading
            {
                SpecReading::Anchor => def
                    .loops
                    .iter()
                    .map(|&id| (id, None, None, spec.get(id).place_of(segment, &[])))
                    .collect(),
                SpecReading::Watch { reads, .. } => {
                    let nth = pick_of(source).map(repeat);
                    reads
                        .iter()
                        .map(|read| match read.place {
                            OccurrencePlace::Named(o) | OccurrencePlace::Chosen(o) => {
                                (read.loop_id, Some(o.index()), nth, read.place)
                            }
                            OccurrencePlace::Unplaced(_) => (read.loop_id, None, nth, read.place),
                        })
                        .collect()
                }
            };
            for (loop_id, occurrence, nth, place) in reads {
                let occurrences = match occurrence {
                    Some(occurrence) => BTreeSet::from([occurrence]),
                    None => engine_occurrences(spec, loop_id, segment, conditions),
                };
                readings.push(Reading {
                    table,
                    column,
                    loop_id,
                    occurrences,
                    place,
                    at,
                    nth,
                });
            }
        }
    }
    readings
}

/// The names of a table and one of its columns.
fn name_of(spec: &Spec, table: usize, column: usize) -> (String, String) {
    let def = &spec.tables()[table];
    (def.name.clone(), def.columns[column].0.clone())
}

/// A place by name.
fn named(
    spec: &Spec,
    (table, column): (usize, usize),
    loop_id: LoopId,
    occurrence: usize,
    (at, nth): (At, Option<usize>),
) -> Place {
    let (table, column) = name_of(spec, table, column);
    let def = spec.get(loop_id);
    Place {
        table,
        column,
        loop_name: def.name.clone(),
        occurrence: def.occurrences[occurrence].name.clone(),
        at,
        nth,
    }
}

/// Every place the projector reads a value from.
fn read_places(spec: &Spec) -> BTreeSet<Place> {
    let mut places = BTreeSet::new();
    for reading in readings(spec) {
        let Some(at) = reading.at else { continue };
        for &occurrence in &reading.occurrences {
            places.insert(named(
                spec,
                (reading.table, reading.column),
                reading.loop_id,
                occurrence,
                (at, reading.nth),
            ));
        }
    }
    places
}

/// Every place the write plan puts a column's value at.
fn written_places(spec: &Spec, plan: &WritePlan) -> BTreeSet<Place> {
    let mut places = BTreeSet::new();
    for (loop_at, loop_plan) in plan.loops.iter().enumerate() {
        for segment in &loop_plan.segments {
            let (table, nth) = match segment.source {
                SegmentSource::Row { table, nth } => (table, Some(nth)),
                SegmentSource::Repeat { table } => (table, None),
                SegmentSource::Envelope => continue,
            };
            for element in &segment.elements {
                let (column, at) = match element.value {
                    ValueSource::Column { column } => (
                        column,
                        At::Element {
                            element: element.element,
                            component: element.component,
                        },
                    ),
                    ValueSource::Group { column, offset } => (
                        column,
                        At::Group {
                            offset,
                            component: element.component,
                        },
                    ),
                    ValueSource::Code(_) => continue,
                };
                places.insert(named(
                    spec,
                    (table, column),
                    LoopId(loop_at),
                    segment.occurrence,
                    (at, nth),
                ));
            }
        }
    }
    places
}

#[test]
fn the_writer_writes_every_column_where_the_reader_reads_it() {
    for version in [Version::V5010, Version::V4010] {
        let spec = builtin(version).spec;
        let plan = WritePlan::new(&spec).expect("a built-in writes");

        let declared = declared_columns(&spec, |source| {
            !matches!(source, ColumnSource::SegmentIndex { .. })
        });
        let read = read_places(&spec);
        let read_columns: BTreeSet<(String, String)> = read
            .iter()
            .map(|place| (place.table.clone(), place.column.clone()))
            .collect();
        assert_eq!(read_columns, declared, "{version:?}: columns read");
        let written = written_places(&spec, &plan);
        assert_eq!(
            read.difference(&written).cloned().collect::<BTreeSet<_>>(),
            BTreeSet::new(),
            "{version:?}: read and not written"
        );
        assert_eq!(
            written.difference(&read).cloned().collect::<BTreeSet<_>>(),
            BTreeSet::new(),
            "{version:?}: written and not read"
        );
    }
}

/// The compat spec: the package over the built-in stack.
fn compat() -> Arc<Spec> {
    match validate(COMPAT, Builtin::packages()) {
        Validation::Yes(valid) => valid.loaded.spec,
        Validation::No(invalid) => panic!("{invalid}"),
    }
}

/// The names of the occurrences of `loop_id` at `occurrences`.
fn names(spec: &Spec, loop_id: LoopId, occurrences: &BTreeSet<usize>) -> BTreeSet<String> {
    let def = spec.get(loop_id);
    occurrences
        .iter()
        .map(|&at| def.occurrences[at].name.clone())
        .collect()
}

/// Every declared column whose source `keep` accepts, as (table, column).
fn declared_columns(spec: &Spec, keep: fn(&ColumnSource) -> bool) -> BTreeSet<(String, String)> {
    spec.tables()
        .iter()
        .flat_map(|def| {
            def.columns
                .iter()
                .filter(|(_, source)| keep(source))
                .map(|(column, _)| (def.name.clone(), column.clone()))
        })
        .collect()
}

/// The occurrences of `loop_id` the write plan places a column in: the one
/// it lands in, or every candidate when several take it.
fn landed(spec: &Spec, loop_id: LoopId, place: OccurrencePlace, segment: &[u8]) -> BTreeSet<usize> {
    let def = spec.get(loop_id);
    match landing(def, place, segment) {
        Ok(at) => BTreeSet::from([at]),
        Err(names) => names
            .iter()
            .filter_map(|name| def.occurrences.iter().position(|found| &found.name == name))
            .collect(),
    }
}

#[test]
fn the_writer_resolves_every_compat_column_where_the_reader_reads_it() {
    let spec = compat();
    let mut differences = Vec::new();
    let mut read_columns = BTreeSet::new();
    for reading in readings(&spec) {
        let def = &spec.tables()[reading.table];
        let (column, source) = &def.columns[reading.column];
        let (segment, _, _, _) = target(def, source);
        let reader = names(&spec, reading.loop_id, &reading.occurrences);
        let writer = names(
            &spec,
            reading.loop_id,
            &landed(&spec, reading.loop_id, reading.place, segment),
        );
        if reader != writer {
            let loop_name = &spec.get(reading.loop_id).name;
            differences.push(format!(
                "{}.{column} in {loop_name}: read {reader:?}, written {writer:?}",
                def.name
            ));
        }
        read_columns.insert((def.name.clone(), column.clone()));
    }
    assert_eq!(differences, Vec::<String>::new());
    assert_eq!(
        read_columns,
        declared_columns(&spec, |_| true),
        "columns read"
    );
}

#[test]
fn every_chosen_occurrence_is_the_one_the_engine_gives() {
    let five = builtin(Version::V5010);
    let four = builtin(Version::V4010);
    let compat = compat();
    let mut failures = Vec::new();
    for spec in [&five.spec, &four.spec, &compat] {
        for (table, def) in spec.tables().iter().enumerate() {
            for (column, place) in spec.places(table).iter().enumerate() {
                let SpecReading::Watch { reads, .. } = &place.reading else {
                    continue;
                };
                let (segment, conditions, _, _) = target(def, &def.columns[column].1);
                for read in reads {
                    let OccurrencePlace::Chosen(chosen) = &read.place else {
                        continue;
                    };
                    let given = engine_occurrences(spec, read.loop_id, segment, conditions);
                    if given != BTreeSet::from([chosen.index()]) {
                        failures.push((
                            def.name.clone(),
                            def.columns[column].0.clone(),
                            spec.get(read.loop_id).name.clone(),
                        ));
                    }
                }
            }
        }
    }
    assert_eq!(failures, Vec::new());
}

/// The refusals the compat spec's tables raise, in order.
const COMPAT_REFUSALS: &str = concat!(
    "spec \"edi_835_parser\" cannot be written (17 reasons)\n",
    "1. table \"rows_organizations\" is anchored on 2 loops (\"1000A\", \"1000B\"); a written row opens one loop instance and nothing in the row says which\n",
    "2. table \"rows_claim_dates\" anchor segment reads \"DTM\" in loop \"2100\", which matches 3 of its occurrences (\"claim_received_date\", \"coverage_expiration_date\", \"statement_from_or_to_date\")\n",
    "3. table \"rows_claim_entities\" anchor segment reads \"NM1\" in loop \"2100\", which matches 7 of its occurrences (\"corrected_patient_insured_name\", \"corrected_priority_payer_name\", \"crossover_carrier_name\", \"insured_name\", \"other_subscriber_name\", \"patient_name\", \"service_provider_name\")\n",
    "4. table \"rows_claim_references\" anchor segment reads \"REF\" in loop \"2100\", which matches 2 of its occurrences (\"other_claim_related_identification\", \"rendering_provider_identification\")\n",
    "5. table \"rows_interchanges\" column \"authorization_information_qualifier\" reads the envelope occurrence \"interchange_control_header\" of loop \"interchange\", which the envelope and the writer's counts give\n",
    "6. table \"rows_interchanges\" column \"receiver\" reads the envelope occurrence \"interchange_control_header\" of loop \"interchange\", which the envelope and the writer's counts give\n",
    "7. table \"rows_interchanges\" column \"sender\" reads the envelope occurrence \"interchange_control_header\" of loop \"interchange\", which the envelope and the writer's counts give\n",
    "8. table \"rows_interchanges\" column \"transmission_date\" reads the envelope occurrence \"interchange_control_header\" of loop \"interchange\", which the envelope and the writer's counts give\n",
    "9. table \"rows_interchanges\" column \"transmission_time\" reads the envelope occurrence \"interchange_control_header\" of loop \"interchange\", which the envelope and the writer's counts give\n",
    "10. table \"rows_references\" anchor segment reads \"REF\" in loop \"2110\", which matches 4 of its occurrences (\"healthcare_policy_identification\", \"line_item_control_number\", \"rendering_provider_information\", \"service_identification\")\n",
    "11. occurrence \"header_number\" (\"LX\") is required in every loop \"2000\", but no column writes it\n",
    "12. occurrence \"patient_name\" (\"NM1\" where NM101 is \"QC\") is required in every loop \"2100\", but no column writes it\n",
    "13. loop \"1000A\" (trigger \"N1\" where {1: \"PR\"}) is required in every \"transaction\", but no table is anchored on it and no column reads it\n",
    "14. loop \"1000B\" (trigger \"N1\" where {1: \"PE\"}) is required in every \"transaction\", but no table is anchored on it and no column reads it\n",
    "15. occurrence \"reassociation_trace_number\" (\"TRN\") is required in every loop \"transaction\", but no column writes it\n",
    "16. required element BPR01 (transaction_handling_code) of occurrence \"financial_information\" in loop \"transaction\", written by table \"rows_payments\", has no column and no single code to write\n",
    "17. required element BPR03 (credit_debit_flag_code) of occurrence \"financial_information\" in loop \"transaction\", written by table \"rows_payments\", has no column and no single code to write",
);

#[test]
fn the_compat_spec_is_refused_for_the_same_reasons() {
    let error = WritePlan::new(&compat()).unwrap_err();
    assert_eq!(error.to_string(), COMPAT_REFUSALS);
}
