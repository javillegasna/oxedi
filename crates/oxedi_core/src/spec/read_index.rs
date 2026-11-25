//! What reading a file needs from the spec per loop and per segment id.

use super::{IdMap, LoopId, Spec, TableDef};

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
        };
        for (at, table) in spec.tables().iter().enumerate() {
            index.add_loops(at, table);
            index.add_resets(spec, at);
            index.add_marks(spec, table);
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
}

impl Spec {
    /// What reading a file needs from the spec per loop and per segment id.
    pub(crate) fn reads(&self) -> &ReadIndex {
        &self.reads
    }
}
