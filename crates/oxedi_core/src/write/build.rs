//! Construction of a [`WritePlan`] from a spec: the inversion of every
//! table column, the loops each table gives instances to, and the checks
//! that refuse a spec whose tables cannot make a valid file.

use std::collections::BTreeMap;

use crate::spec::{ColumnSource, LoopId, OccurrenceDef, Pick, Spec, TableDef};

use super::plan::{
    ElementPlan, Instances, LoopPlan, SegmentPlan, SegmentSource, ValueSource, WritePlan,
};
use super::refusal::{PlanError, Refusal, render_source};
use super::required::add_qualifier;

/// A column value that lands in one segment: which column, which repeat
/// and which element.
#[derive(Debug, Clone)]
pub(super) struct Entry {
    pub(super) table: usize,
    pub(super) column: usize,
    pub(super) pick: Pick,
    pub(super) element: usize,
    pub(super) component: Option<usize>,
}

/// The segments a loop's columns write, keyed by loop, occurrence and the
/// codes their `where` fixes beyond the occurrence's own qualifier.
pub(super) type Groups = BTreeMap<(usize, usize, Codes), Vec<Entry>>;

/// `(1-based element position, code)` pairs a `where` fixes, by position.
pub(super) type Codes = Vec<(usize, Vec<u8>)>;

pub(super) struct Builder<'s> {
    pub(super) spec: &'s Spec,
    pub(super) loops: Vec<LoopPlan>,
    pub(super) groups: Groups,
    pub(super) refusals: Vec<Refusal>,
}

impl WritePlan {
    /// Compiles the plan that writes `spec`'s tables back into a file, or
    /// every reason the spec's tables cannot make a valid one.
    pub fn new(spec: &Spec) -> Result<WritePlan, PlanError> {
        let mut builder = Builder {
            spec,
            loops: vec![
                LoopPlan {
                    instances: Instances::Absent,
                    segments: Vec::new(),
                };
                spec.loops().len()
            ],
            groups: Groups::new(),
            refusals: Vec::new(),
        };
        builder.envelope();
        for (index, table) in spec.tables().iter().enumerate() {
            if table.segment.is_none() {
                builder.anchor(index, table);
            }
        }
        for (index, table) in spec.tables().iter().enumerate() {
            match table.segment {
                Some(_) => builder.repeat_table(index, table),
                None => builder.loop_table(index, table),
            }
        }
        builder.segments();
        builder.implied();
        builder.required();
        builder.sort();
        if builder.refusals.is_empty() {
            Ok(WritePlan {
                loops: builder.loops,
            })
        } else {
            Err(PlanError {
                spec: spec.name().to_string(),
                refusals: builder.refusals,
            })
        }
    }
}

impl Builder<'_> {
    pub(super) fn table_name(&self, table: usize) -> String {
        self.spec
            .tables()
            .get(table)
            .map(|def| def.name.clone())
            .unwrap_or_default()
    }

    pub(super) fn column_name(&self, table: usize, column: usize) -> String {
        self.spec
            .tables()
            .get(table)
            .and_then(|def| def.columns.get(column))
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
    }

    pub(super) fn occurrence(&self, id: usize, occurrence: usize) -> Option<&OccurrenceDef> {
        self.spec.loops().get(id)?.occurrences.get(occurrence)
    }

    /// The trigger and end of every loop with a `control` come from the
    /// envelope; a loop no table anchors gets its instances from it too.
    fn envelope(&mut self) {
        for (index, def) in self.spec.loops().iter().enumerate() {
            if def.control.is_none() {
                continue;
            }
            let Some(plan) = self.loops.get_mut(index) else {
                continue;
            };
            plan.instances = Instances::Envelope;
            if !def.occurrences.is_empty() {
                plan.segments.push(SegmentPlan {
                    occurrence: 0,
                    source: SegmentSource::Envelope,
                    elements: Vec::new(),
                });
            }
        }
    }

    /// Gives the loop `id` its instances from `kind`, or refuses the column
    /// when something else already gives them; `false` after a refusal.
    fn give_instances(&mut self, id: LoopId, kind: Instances, table: usize, column: usize) -> bool {
        let Some(plan) = self.loops.get_mut(id.index()) else {
            return false;
        };
        let writer = match plan.instances {
            Instances::Absent => {
                plan.instances = kind;
                return true;
            }
            current if current == kind => return true,
            Instances::Envelope => "the envelope".to_string(),
            Instances::Rows { table: other }
            | Instances::Inside { table: other }
            | Instances::Groups { table: other } => {
                format!("table {:?}", self.table_name(other))
            }
            Instances::Implied => return true,
        };
        self.refusals.push(Refusal::WrittenByAnotherTable {
            table: self.table_name(table),
            column: self.column_name(table, column),
            loop_name: self.spec.loop_name(id).to_string(),
            writer,
        });
        false
    }

    /// The occurrence of loop `id` that `segment` and `conditions` select,
    /// with the conditions left once the occurrence's own single-code
    /// qualifier is taken out; every occurrence that matches when there is
    /// not exactly one.
    fn resolve(
        &self,
        id: LoopId,
        segment: &[u8],
        conditions: &[(usize, Vec<u8>)],
    ) -> Result<(usize, Codes), Vec<String>> {
        let def = self.spec.get(id);
        let accepts = |occurrence: &OccurrenceDef| {
            occurrence.segment == segment
                && conditions.iter().all(|(position, value)| {
                    let listed = |codes: &[String]| codes.iter().any(|c| c.as_bytes() == value);
                    match &occurrence.qualifier {
                        Some(q) if q.element == *position && q.component.is_none() => {
                            listed(&q.codes)
                        }
                        _ => occurrence
                            .codes
                            .get(&(*position, None))
                            .is_none_or(|codes| listed(codes)),
                    }
                })
        };
        let matched: Vec<usize> = def
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| accepts(occurrence))
            .map(|(index, _)| index)
            .collect();
        let [index] = matched.as_slice() else {
            return Err(matched
                .iter()
                .filter_map(|&index| def.occurrences.get(index))
                .map(|occurrence| occurrence.name.clone())
                .collect());
        };
        let occurrence = def.occurrences.get(*index);
        let implied = occurrence
            .and_then(|o| o.qualifier.as_ref())
            .filter(|q| q.component.is_none() && q.codes.len() == 1)
            .map(|q| q.element);
        let rest = conditions
            .iter()
            .filter(|(position, _)| Some(*position) != implied)
            .cloned()
            .collect();
        Ok((*index, rest))
    }

    /// A table with one row per instance of its anchor loops.
    /// A table without a segment gives one instance of its anchor loops per row.
    fn anchor(&mut self, index: usize, table: &TableDef) {
        for &anchor in &table.loops {
            if let Some(plan) = self.loops.get_mut(anchor.index()) {
                plan.instances = Instances::Rows { table: index };
            }
        }
    }

    /// The columns of a table with one row per instance of its anchor loops.
    fn loop_table(&mut self, index: usize, table: &TableDef) {
        for (first, &anchor) in table.loops.iter().enumerate() {
            for (column, (name, source)) in table.columns.iter().enumerate() {
                let ColumnSource::Element {
                    loop_id,
                    segment,
                    conditions,
                    occurrence,
                    pick,
                    element,
                    component,
                } = source
                else {
                    continue;
                };
                // A column that names its loop reads that loop whatever the
                // anchor: it is placed once.
                if loop_id.is_some() && first > 0 {
                    continue;
                }
                let reader = loop_id.unwrap_or(anchor);
                let resolved = match occurrence {
                    Some(name) => self
                        .spec
                        .get(reader)
                        .occurrences
                        .iter()
                        .position(|o| o.name == *name)
                        .map(|at| (at, Vec::new()))
                        .ok_or_else(Vec::new),
                    None => self.resolve(reader, segment, conditions),
                };
                let (at, codes) = match resolved {
                    Ok(found) => found,
                    Err(candidates) => {
                        self.refusals.push(Refusal::AmbiguousSource {
                            table: table.name.clone(),
                            column: Some(name.clone()),
                            loop_name: self.spec.loop_name(reader).to_string(),
                            source: render_source(segment, conditions),
                            candidates,
                        });
                        continue;
                    }
                };
                let def = self.spec.get(reader);
                if def.control.is_some() && at == 0 {
                    self.refusals.push(Refusal::EnvelopeColumn {
                        table: table.name.clone(),
                        column: name.clone(),
                        loop_name: def.name.clone(),
                        occurrence: def
                            .occurrences
                            .first()
                            .map(|o| o.name.clone())
                            .unwrap_or_default(),
                    });
                    continue;
                }
                if let Some(id) = loop_id {
                    let kind = if self.spec.ancestors(anchor).contains(id) {
                        Instances::Groups { table: index }
                    } else {
                        Instances::Inside { table: index }
                    };
                    if !self.give_instances(*id, kind, index, column) {
                        continue;
                    }
                }
                self.groups
                    .entry((reader.index(), at, codes))
                    .or_default()
                    .push(Entry {
                        table: index,
                        column,
                        pick: *pick,
                        element: *element,
                        component: *component,
                    });
            }
        }
    }

    /// A table with one row per element group of a segment in its anchor loops.
    fn repeat_table(&mut self, index: usize, table: &TableDef) {
        let Some(segment) = &table.segment else {
            return;
        };
        for &anchor in &table.loops {
            let at = match self.resolve(anchor, segment, &[]) {
                Ok((at, _)) => at,
                Err(candidates) => {
                    self.refusals.push(Refusal::AmbiguousSource {
                        table: table.name.clone(),
                        column: None,
                        loop_name: self.spec.loop_name(anchor).to_string(),
                        source: render_source(segment, &[]),
                        candidates,
                    });
                    continue;
                }
            };
            let from = table.repeat.map_or(1, |repeat| repeat.from);
            let mut elements = Vec::new();
            for (column, (_, source)) in table.columns.iter().enumerate() {
                match source {
                    ColumnSource::Element {
                        element, component, ..
                    } => elements.push(ElementPlan {
                        element: *element,
                        component: *component,
                        value: ValueSource::Column { column },
                    }),
                    ColumnSource::GroupElement { offset, component } => {
                        elements.push(ElementPlan {
                            element: from.saturating_add(*offset),
                            component: *component,
                            value: ValueSource::Group {
                                column,
                                offset: *offset,
                            },
                        });
                    }
                    ColumnSource::SegmentIndex { .. } => {}
                }
            }
            let mut plan = SegmentPlan {
                occurrence: at,
                source: SegmentSource::Repeat { table: index },
                elements,
            };
            if let Some(occurrence) = self.occurrence(anchor.index(), at) {
                add_qualifier(&mut plan, occurrence);
            }
            if let Some(loop_plan) = self.loops.get_mut(anchor.index()) {
                loop_plan.segments.push(plan);
            }
        }
    }

    /// Every loop above a written loop is written around it.
    fn implied(&mut self) {
        for index in 0..self.loops.len() {
            let written = self
                .loops
                .get(index)
                .is_some_and(|plan| plan.instances != Instances::Absent);
            if !written {
                continue;
            }
            let mut above = self.spec.loops().get(index).and_then(|def| def.parent);
            while let Some(id) = above {
                if let Some(plan) = self.loops.get_mut(id.index())
                    && plan.instances == Instances::Absent
                {
                    plan.instances = Instances::Implied;
                }
                above = self.spec.get(id).parent;
            }
        }
    }

    fn sort(&mut self) {
        let spec = self.spec;
        for (index, plan) in self.loops.iter_mut().enumerate() {
            let Some(def) = spec.loops().get(index) else {
                continue;
            };
            let pos = |at: usize| def.occurrences.get(at).map_or(0, |o| o.pos);
            plan.segments
                .sort_by_key(|segment| (pos(segment.occurrence), segment.occurrence));
            for segment in &mut plan.segments {
                segment
                    .elements
                    .sort_by_key(|element| (element.element, element.component));
            }
        }
    }
}
