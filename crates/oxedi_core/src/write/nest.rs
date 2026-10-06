//! The rows of each table nested under the rows above them, by their
//! reference columns: which rows a parent row holds, in row order, and in
//! which loop instance each row of a table anchored on a segment goes.

use std::collections::HashMap;

use crate::spec::{LoopId, Spec};

use super::data::Data;
use super::finding::Finding;
use super::plan::{Instances, WritePlan};

/// Where every row goes.
#[derive(Debug, Clone, Default)]
pub(super) struct Nest {
    /// By table: the rows each row of its parent table holds, by the
    /// parent's position; a table with no parent has one list, at 0.
    pub(super) children: Vec<Vec<Vec<usize>>>,
    /// By table anchored on a segment: its rows by `(anchor loop, position
    /// of the row whose instance holds them)`.
    pub(super) repeats: Vec<HashMap<(usize, usize), Vec<usize>>>,
    /// By table anchored on a segment, by row: the table whose reference
    /// placed the row.
    pub(super) placed_by: Vec<Vec<Option<usize>>>,
}

impl Nest {
    /// The rows of `table` that the row at `parent` (of its parent table)
    /// holds; every row for a table with no parent.
    pub(super) fn rows(&self, table: usize, parent: Option<usize>) -> &[usize] {
        self.children
            .get(table)
            .and_then(|lists| lists.get(parent.unwrap_or(0)))
            .map_or(&[], Vec::as_slice)
    }

    /// The rows of `table` that go in the instance of `anchor` opened by
    /// the row at `owner`.
    pub(super) fn repeat_rows(&self, table: usize, anchor: usize, owner: usize) -> &[usize] {
        self.repeats
            .get(table)
            .and_then(|map| map.get(&(anchor, owner)))
            .map_or(&[], Vec::as_slice)
    }
}

/// Nests every row, and reports the rows that cannot be placed and the row
/// numbers two rows of a table that others refer to share.
pub(super) fn nest(
    spec: &Spec,
    plan: &WritePlan,
    data: &[Data<'_>],
    findings: &mut Vec<Finding>,
) -> Nest {
    let tables = spec.tables();
    let keys: Vec<HashMap<i64, usize>> = data
        .iter()
        .enumerate()
        .map(|(index, table)| {
            let referred = tables.iter().any(|def| def.ancestors.contains(&index));
            if !referred {
                return HashMap::new();
            }
            let mut keys = HashMap::with_capacity(table.rows);
            for row in 0..table.rows {
                let key = table.key(row);
                if let Some(&first) = keys.get(&key) {
                    findings.push(Finding::DuplicateRowNumber {
                        table: table.name.clone(),
                        row,
                        value: key,
                        first,
                    });
                } else {
                    keys.insert(key, row);
                }
            }
            keys
        })
        .collect();
    let mut nest = Nest {
        children: vec![Vec::new(); tables.len()],
        repeats: vec![HashMap::new(); tables.len()],
        placed_by: vec![Vec::new(); tables.len()],
    };
    for (index, def) in tables.iter().enumerate() {
        let Some(table) = data.get(index) else {
            continue;
        };
        if def.segment.is_some() {
            // The deepest anchor first: a row with a reference to a row
            // anchored deeper goes in that row's instance.
            let mut anchors: Vec<(LoopId, usize)> = def
                .loops
                .iter()
                .filter_map(|&anchor| match plan.loops.get(anchor.index())?.instances {
                    Instances::Rows { table: owner } => Some((anchor, owner)),
                    _ => None,
                })
                .collect();
            anchors.sort_by_key(|&(anchor, _)| std::cmp::Reverse(spec.ancestors(anchor).len()));
            let mut placed = Vec::with_capacity(table.rows);
            for row in 0..table.rows {
                let mut place = None;
                let mut last = None;
                for &(anchor, owner) in &anchors {
                    last = Some(owner);
                    let Some(value) = table.reference(owner, row) else {
                        continue;
                    };
                    place = Some(
                        keys.get(owner)
                            .and_then(|keys| keys.get(&value))
                            .map(|&at| (anchor.index(), owner, at))
                            .ok_or((owner, Some(value))),
                    );
                    break;
                }
                let place = place.unwrap_or(Err((last.unwrap_or(index), None)));
                match place {
                    Ok((anchor, owner, at)) => {
                        if let Some(map) = nest.repeats.get_mut(index) {
                            map.entry((anchor, at)).or_default().push(row);
                        }
                        placed.push(Some(owner));
                    }
                    Err((owner, value)) => {
                        findings.push(missing(spec, table, row, owner, value));
                        placed.push(None);
                    }
                }
            }
            if let Some(slot) = nest.placed_by.get_mut(index) {
                *slot = placed;
            }
            continue;
        }
        let Some(parent) = def.parent else {
            if let Some(slot) = nest.children.get_mut(index) {
                *slot = vec![(0..table.rows).collect()];
            }
            continue;
        };
        let parent_rows = data.get(parent).map_or(0, |data| data.rows);
        let mut lists = vec![Vec::new(); parent_rows];
        for row in 0..table.rows {
            let value = table.reference(parent, row);
            let at = value.and_then(|value| keys.get(parent)?.get(&value).copied());
            match at.and_then(|at| lists.get_mut(at)) {
                Some(list) => list.push(row),
                None => findings.push(missing(spec, table, row, parent, value)),
            }
        }
        if let Some(slot) = nest.children.get_mut(index) {
            *slot = lists;
        }
    }
    nest
}

fn missing(
    spec: &Spec,
    table: &Data<'_>,
    row: usize,
    parent: usize,
    value: Option<i64>,
) -> Finding {
    let def = spec.tables().get(parent);
    Finding::MissingParent {
        table: table.name.clone(),
        row,
        column: def.map(|def| def.reference.clone()).unwrap_or_default(),
        value,
        parent: def.map(|def| def.name.clone()).unwrap_or_default(),
    }
}
