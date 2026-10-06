//! Construction of a [`WritePlan`] from a spec: the inversion of every
//! table column, the loops each table gives instances to, and the checks
//! that refuse a spec whose tables cannot make a valid file.

use std::collections::BTreeMap;

use crate::spec::{ColumnSource, LoopId, OccurrenceDef, Pick, Spec, TableDef};

use super::codes::Resolved;
use super::plan::{
    ElementPlan, Instances, LoopPlan, SegmentPlan, SegmentSource, Unwritten, ValueSource, WritePlan,
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

/// The segments a loop's columns write, keyed by loop, occurrence, table and
/// the codes their `where` fixes beyond the occurrence's own qualifier.
pub(super) type Groups = BTreeMap<(usize, usize, usize, Codes), Vec<Entry>>;

/// `(1-based element position, code)` pairs a `where` fixes, by position.
pub(super) type Codes = Vec<(usize, Vec<u8>)>;

pub(super) struct Builder<'s> {
    pub(super) spec: &'s Spec,
    pub(super) loops: Vec<LoopPlan>,
    pub(super) groups: Groups,
    pub(super) refusals: Vec<Refusal>,
    /// Tables refused for their anchor: their columns are not placed.
    refused: Vec<usize>,
    /// `(table, anchor loop)` of every table anchored on a segment.
    pub(super) repeats: Vec<(usize, LoopId)>,
    unwritten: Vec<Unwritten>,
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
            refused: Vec::new(),
            repeats: Vec::new(),
            unwritten: Vec::new(),
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
        builder.repeats_placed();
        builder.required();
        builder.sort();
        if builder.refusals.is_empty() {
            Ok(WritePlan {
                loops: builder.loops,
                unwritten: builder.unwritten,
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
            if let Some(occurrence) = def.trigger_occurrence() {
                plan.segments.push(SegmentPlan {
                    occurrence,
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

    /// A table without a segment gives one instance of its anchor loop per
    /// row, or is refused when another table already gives that loop its
    /// rows, or when it has several anchor loops: nothing in a row says
    /// which of them it opens.
    fn anchor(&mut self, index: usize, table: &TableDef) {
        if table.loops.len() > 1 {
            self.refusals.push(Refusal::SeveralAnchorLoops {
                table: table.name.clone(),
                loops: table
                    .loops
                    .iter()
                    .map(|&id| self.spec.loop_name(id).to_string())
                    .collect(),
            });
            self.refused.push(index);
            return;
        }
        for &anchor in &table.loops {
            let Some(plan) = self.loops.get_mut(anchor.index()) else {
                continue;
            };
            match plan.instances {
                Instances::Rows { table: other } if other != index => {
                    self.refusals.push(Refusal::AnchoredByAnotherTable {
                        table: table.name.clone(),
                        loop_name: self.spec.loop_name(anchor).to_string(),
                        other: self.table_name(other),
                    });
                    self.refused.push(index);
                    return;
                }
                _ => plan.instances = Instances::Rows { table: index },
            }
        }
    }

    /// The columns of a table with one row per instance of its anchor loop
    /// (a table with several is refused by [`Builder::anchor`]).
    fn loop_table(&mut self, index: usize, table: &TableDef) {
        if self.refused.contains(&index) {
            return;
        }
        if let Some(&anchor) = table.loops.first() {
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
                let reader = loop_id.unwrap_or(anchor);
                let resolved = match occurrence {
                    Some(name) => self
                        .spec
                        .get(reader)
                        .occurrences
                        .iter()
                        .position(|o| o.name == *name)
                        .map_or(Resolved::Candidates(Vec::new()), |at| {
                            Resolved::One(at, Vec::new())
                        }),
                    None => self.resolve(reader, segment, conditions),
                };
                let (at, codes) = match resolved {
                    Resolved::One(at, codes) => (at, codes),
                    Resolved::Nothing(excluded) => {
                        self.unwritten.push(Unwritten {
                            table: index,
                            column,
                            place: excluded.place,
                            code: excluded.code,
                            codes: excluded.codes,
                        });
                        continue;
                    }
                    Resolved::Candidates(candidates) => {
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
                if def.control.is_some() && def.trigger_occurrence() == Some(at) {
                    self.refusals.push(Refusal::EnvelopeColumn {
                        table: table.name.clone(),
                        column: name.clone(),
                        loop_name: def.name.clone(),
                        occurrence: def
                            .occurrences
                            .get(at)
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
                    .entry((reader.index(), at, index, codes))
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
            self.repeats.push((index, anchor));
            let at = match self.resolve(anchor, segment, &[]) {
                Resolved::One(at, _) => at,
                Resolved::Nothing(_) => continue,
                Resolved::Candidates(candidates) => {
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
            self.check_codes(anchor.index(), &plan, index, None);
            if let Some(loop_plan) = self.loops.get_mut(anchor.index()) {
                loop_plan.segments.push(plan);
            }
        }
    }
}
