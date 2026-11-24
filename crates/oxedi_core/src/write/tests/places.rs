//! The places the projector reads each column from, against the places the
//! write plan writes it to, over the built-in specs.

use std::collections::{BTreeMap, BTreeSet};

use crate::delimiters::test_support::plain;
use crate::project::Projector;
use crate::spec::test_support::{builtin, segs};
use crate::spec::{ColumnSource, LoopId, Qualifier, Spec, TableDef};
use crate::spec_load::Version;
use crate::vocabulary::Pick;
use crate::write::{SegmentSource, ValueSource, WritePlan};

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

/// The segment and conditions a column reads, and what of the segment.
type Target<'s> = (&'s [u8], &'s [(usize, Vec<u8>)], Option<At>);

fn target<'s>(def: &'s TableDef, source: &'s ColumnSource) -> Target<'s> {
    match source {
        ColumnSource::Element {
            segment,
            conditions,
            element,
            component,
            ..
        } => {
            let at = At::Element {
                element: *element,
                component: *component,
            };
            (segment, conditions, Some(at))
        }
        ColumnSource::SegmentIndex {
            segment,
            conditions,
            ..
        } => (segment, conditions, None),
        ColumnSource::GroupElement { offset, component } => {
            let at = At::Group {
                offset: *offset,
                component: *component,
            };
            (def.segment.as_deref().unwrap_or_default(), &[], Some(at))
        }
    }
}

/// Every column the projector reads, with the occurrences it reads it in.
fn readings(spec: &Spec) -> Vec<Reading> {
    let mut readings = Vec::new();
    for place in Projector::new(spec, &plain()).read_places() {
        let def = &spec.tables()[place.table];
        let (_, source) = &def.columns[place.column];
        let (segment, conditions, at) = target(def, source);
        let occurrences = match place.occurrence {
            Some(occurrence) => BTreeSet::from([occurrence]),
            None => engine_occurrences(spec, place.loop_id, segment, conditions),
        };
        readings.push(Reading {
            table: place.table,
            column: place.column,
            loop_id: place.loop_id,
            occurrences,
            at,
            nth: place.pick.map(repeat),
        });
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

/// The 4010 payer technical contact: its columns read `PER` with PER01 = BL in
/// loop 1000A, a code the 4010 list of PER01 (CX) leaves out. The reader fills
/// them from the loop's one `PER` occurrence; the writer leaves them unwritten.
/// `(column, element)`, in column order.
const TECHNICAL_CONTACT_4010: [(&str, usize); 3] = [
    ("payer_technical_contact_name", 2),
    ("payer_technical_contact_number", 4),
    ("payer_technical_contact_qualifier", 3),
];

#[test]
fn the_writer_writes_every_column_where_the_reader_reads_it() {
    for (version, contact) in [
        (Version::V5010, &[][..]),
        (Version::V4010, &TECHNICAL_CONTACT_4010[..]),
    ] {
        let spec = builtin(version).spec;
        let plan = WritePlan::new(&spec).expect("a built-in writes");

        let declared: BTreeSet<(String, String)> = spec
            .tables()
            .iter()
            .flat_map(|def| {
                def.columns
                    .iter()
                    .filter(|(_, source)| !matches!(source, ColumnSource::SegmentIndex { .. }))
                    .map(|(column, _)| (def.name.clone(), column.clone()))
            })
            .collect();
        let read = read_places(&spec);
        let read_columns: BTreeSet<(String, String)> = read
            .iter()
            .map(|place| (place.table.clone(), place.column.clone()))
            .collect();
        assert_eq!(read_columns, declared, "{version:?}: columns read");
        let written = written_places(&spec, &plan);
        let expected: BTreeSet<Place> = contact
            .iter()
            .map(|&(column, element)| Place {
                table: "payments".to_string(),
                column: column.to_string(),
                loop_name: "1000A".to_string(),
                occurrence: "payer_business_contact_information".to_string(),
                at: At::Element {
                    element,
                    component: None,
                },
                nth: Some(1),
            })
            .collect();
        assert_eq!(
            read.difference(&written).cloned().collect::<BTreeSet<_>>(),
            expected,
            "{version:?}: read and not written"
        );
        assert_eq!(
            written.difference(&read).cloned().collect::<BTreeSet<_>>(),
            BTreeSet::new(),
            "{version:?}: written and not read"
        );

        let unwritten: Vec<String> = plan
            .unwritten
            .iter()
            .map(|entry| {
                let (_, column) = name_of(&spec, entry.table, entry.column);
                format!("{column} {} {} {:?}", entry.place, entry.code, entry.codes)
            })
            .collect();
        let expected: Vec<String> = contact
            .iter()
            .map(|(column, _)| format!("{column} PER01 BL [\"CX\"]"))
            .collect();
        assert_eq!(unwritten, expected, "{version:?}: unwritten");
    }
}
