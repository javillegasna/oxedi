//! What reading a file needs from the spec per loop and per segment id.

use super::{
    ColumnSource, IdMap, LoopId, OccurrencePlace, Reading, Relation, Spec, TableDef, Target,
};
use crate::value::ColumnType;
use crate::vocabulary::Pick;

/// What of a captured segment a watcher reads. A group column belongs to a
/// table anchored on a segment, which no watcher reads, so it has no variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadTarget {
    /// 1-based element, and 1-based component when it reads one.
    Element {
        element: usize,
        component: Option<usize>,
    },
    /// The 0-based index of the segment in the stream.
    Index,
}

impl ReadTarget {
    /// The read target of a column's target; `None` for a group column.
    fn of(target: Target) -> Option<Self> {
        match target {
            Target::Element { element, component } => Some(Self::Element { element, component }),
            Target::Index => Some(Self::Index),
            Target::Group { .. } => None,
        }
    }
}

/// A column of a loop table reading the segments of one id captured in one
/// loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Watcher {
    /// 0-based index into the spec's tables.
    pub(crate) table: usize,
    /// 0-based index into the table's declared columns.
    pub(crate) column: usize,
    /// The occurrence the column reads in the loop.
    pub(crate) place: OccurrencePlace,
    /// `(1-based element, value)` a segment must hold, from the column's
    /// source.
    pub(crate) conditions: Vec<(usize, Vec<u8>)>,
    /// Which matching segment gives the value.
    pub(crate) pick: Pick,
    /// Where the loop sits relative to the table's anchor.
    pub(crate) relation: Relation,
    /// What of the segment the column reads.
    pub(crate) target: ReadTarget,
    /// The type of the values the column holds.
    pub(crate) kind: ColumnType,
}

/// What reading a file needs from the spec per loop and per segment id,
/// built once from the places.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadIndex {
    /// Per loop: the table without `segment` anchored in it.
    anchored: Vec<Option<usize>>,
    /// Per loop: the tables anchored on a segment inside it, in table order.
    segment_tables: Vec<Vec<usize>>,
    /// Per loop: `(table, carried slot)` of the values read from it, which
    /// start over when an instance of it opens.
    resets: Vec<Vec<(usize, usize)>>,
    /// Per segment id: the `(element, component)` positions some column
    /// reads, sorted and without repeats.
    read: IdMap<Vec<(usize, Option<usize>)>>,
    /// Per segment id, per loop: the columns the segment feeds when that
    /// loop captures it.
    watchers: IdMap<Vec<Vec<Watcher>>>,
}

impl ReadIndex {
    /// The index of `spec`, whose places are compiled.
    pub(crate) fn build(spec: &Spec) -> Self {
        let loops = spec.loops().len();
        let mut index = Self {
            anchored: vec![None; loops],
            segment_tables: vec![Vec::new(); loops],
            resets: vec![Vec::new(); loops],
            read: IdMap::default(),
            watchers: IdMap::default(),
        };
        for (at, table) in spec.tables().iter().enumerate() {
            index.add_loops(at, table);
            index.add_resets(spec, at);
            index.add_marks(spec, table);
            index.add_watchers(spec, at, table);
        }
        index
    }

    /// Records the loops table `at` is anchored in.
    fn add_loops(&mut self, at: usize, table: &TableDef) {
        for &id in &table.loops {
            match table.segment {
                Some(_) => self.segment_tables[id.index()].push(at),
                None => self.anchored[id.index()] = Some(at),
            }
        }
    }

    /// Records the loops whose opening resets a carried value of table `at`.
    /// An `Above` column reads exactly its one named loop.
    fn add_resets(&mut self, spec: &Spec, at: usize) {
        for (slot, reads) in spec.places(at).iter().filter_map(|p| p.reading.above()) {
            for read in reads {
                self.resets[read.loop_id.index()].push((at, slot));
            }
        }
    }

    /// Records every position a column of `table` reads, keeping each id's
    /// positions sorted and without repeats.
    fn add_marks(&mut self, spec: &Spec, table: &TableDef) {
        for (_, source) in &table.columns {
            for (id, element, component) in spec.positions(table, source) {
                let read = self.read.entry_or_insert_with(id, Vec::new);
                if let Err(slot) = read.binary_search(&(element, component)) {
                    read.insert(slot, (element, component));
                }
            }
        }
    }

    /// Records the columns of loop table `at` on the segment ids they read,
    /// in each loop they read them in.
    fn add_watchers(&mut self, spec: &Spec, at: usize, table: &TableDef) {
        if table.segment.is_some() {
            return;
        }
        let loops = spec.loops().len();
        for (column, (place, (_, source))) in spec.places(at).iter().zip(&table.columns).enumerate()
        {
            let (
                Reading::Watch { relation, reads },
                Some((segment, conditions, pick)),
                Some(target),
            ) = (
                &place.reading,
                source_read(source),
                ReadTarget::of(place.target),
            )
            else {
                continue;
            };
            let per_loop = self
                .watchers
                .entry_or_insert_with(segment, || vec![Vec::new(); loops]);
            for read in reads {
                if let Some(list) = per_loop.get_mut(read.loop_id.index()) {
                    list.push(Watcher {
                        table: at,
                        column,
                        place: read.place,
                        conditions: conditions.to_vec(),
                        pick,
                        relation: *relation,
                        target,
                        kind: place.kind,
                    });
                }
            }
        }
    }

    /// The table without `segment` anchored in loop `id`.
    pub(crate) fn anchored(&self, id: LoopId) -> Option<usize> {
        self.anchored.get(id.index()).copied().flatten()
    }

    /// The tables anchored on a segment inside loop `id`, in table order.
    pub(crate) fn segment_tables(&self, id: LoopId) -> &[usize] {
        self.segment_tables
            .get(id.index())
            .map_or(&[], Vec::as_slice)
    }

    /// `(table, carried slot)` of the values read from loop `id`.
    pub(crate) fn resets(&self, id: LoopId) -> &[(usize, usize)] {
        self.resets.get(id.index()).map_or(&[], Vec::as_slice)
    }

    /// Whether some column reads `element` (1-based), or its 1-based
    /// `component`, of the segments with `id`. A composite read whole marks
    /// no component.
    pub(crate) fn is_read(&self, id: &[u8], element: usize, component: Option<usize>) -> bool {
        self.read
            .get(id)
            .is_some_and(|read| read.binary_search(&(element, component)).is_ok())
    }

    /// The columns the segments with `id` feed when loop `loop_id` captures
    /// them, in table then column order.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the projector reads it in the next change")
    )]
    pub(crate) fn watchers(&self, loop_id: LoopId, id: &[u8]) -> &[Watcher] {
        self.watchers
            .get(id)
            .and_then(|per_loop| per_loop.get(loop_id.index()))
            .map_or(&[], Vec::as_slice)
    }
}

/// A column's `(1-based element, value)` conditions.
type Conditions = [(usize, Vec<u8>)];

/// The segment id, conditions and pick of a column that reads a captured
/// segment.
fn source_read(source: &ColumnSource) -> Option<(&[u8], &Conditions, Pick)> {
    match source {
        ColumnSource::Element {
            segment,
            conditions,
            pick,
            ..
        }
        | ColumnSource::SegmentIndex {
            segment,
            conditions,
            pick,
            ..
        } => Some((segment.as_slice(), conditions, *pick)),
        ColumnSource::GroupElement { .. } => None,
    }
}

impl Spec {
    /// What reading a file needs from the spec per loop and per segment id.
    pub(crate) fn reads(&self) -> &ReadIndex {
        &self.reads
    }
}
