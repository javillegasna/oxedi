//! Construction of a [`WritePlan`] from a spec: the inversion of every
//! table column, the loops each table gives instances to, and the checks
//! that refuse a spec whose tables cannot make a valid file.

use std::collections::BTreeMap;

use crate::spec::{
    ColumnPlace, ColumnSource, LoopId, LoopRead, OccurrenceDef, OccurrencePlace, Reading, Relation,
    Spec, TableDef, Target,
};
use crate::vocabulary::Pick;

use super::codes::{Resolved, landing};
use super::plan::{
    ElementPlan, Instances, LoopPlan, SegmentPlan, SegmentSource, Unwritten, ValueSource, WritePlan,
};
use super::refusal::{PlanError, Refusal, render_source};
use super::required::add_qualifier;

/// A refusal carried as the `Err` of a check, boxed to keep the `Result` small.
type Refused = Box<Refusal>;

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
        let mut builder = Builder::new(spec);
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

impl<'s> Builder<'s> {
    /// A builder with every loop absent and nothing placed or refused yet.
    pub(super) fn new(spec: &'s Spec) -> Self {
        Builder {
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

    pub(super) fn loop_name(&self, id: usize) -> String {
        self.spec
            .loops()
            .get(id)
            .map(|def| def.name.clone())
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
    /// when something else already gives them.
    fn give_instances(
        &mut self,
        id: LoopId,
        kind: Instances,
        table: usize,
        column: usize,
    ) -> Result<(), Refused> {
        // Every loop has a plan, so the missing case cannot happen and asks
        // nothing of the column.
        let Some(plan) = self.loops.get_mut(id.index()) else {
            return Ok(());
        };
        let writer = match plan.instances {
            Instances::Absent => {
                plan.instances = kind;
                return Ok(());
            }
            current if current == kind => return Ok(()),
            Instances::Envelope => "the envelope".to_string(),
            Instances::Rows { table: other }
            | Instances::Inside { table: other }
            | Instances::Groups { table: other } => {
                format!("table {:?}", self.table_name(other))
            }
            Instances::Implied => return Ok(()),
        };
        Err(Box::new(Refusal::WrittenByAnotherTable {
            table: self.table_name(table),
            column: self.column_name(table, column),
            loop_name: self.spec.loop_name(id).to_string(),
            writer,
        }))
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
    /// (a table with several is refused by [`Builder::anchor`]). Each column
    /// lands through checks that return its refusal as a value.
    fn loop_table(&mut self, index: usize, table: &TableDef) {
        if self.refused.contains(&index) {
            return;
        }
        let places = self.spec.places(index);
        let writes = table
            .columns
            .iter()
            .zip(places)
            .enumerate()
            .filter_map(|(column, (source, place))| ColumnWrite::of(column, source, place));
        for write in writes {
            if let Err(refusal) = self.land(index, table, &write) {
                self.refusals.push(*refusal);
            }
        }
    }

    /// Places one column of table `index` in its occurrence: the group entry
    /// of the segment it writes, and the instances its loop needs.
    fn land(
        &mut self,
        index: usize,
        table: &TableDef,
        write: &ColumnWrite<'_>,
    ) -> Result<(), Refused> {
        let reader = write.read.loop_id;
        let Some((at, codes)) = self.occurrence_of(index, table, write)? else {
            return Ok(());
        };
        self.not_the_envelope(table, write, at)?;
        if let Some(kind) = instances(write.relation, index) {
            self.give_instances(reader, kind, index, write.column)?;
        }
        self.groups
            .entry((reader.index(), at, index, codes))
            .or_default()
            .push(Entry {
                table: index,
                column: write.column,
                pick: write.pick,
                element: write.element,
                component: write.component,
            });
        Ok(())
    }

    /// The occurrence a column writes and the codes left to fix once its
    /// qualifier is taken out; `None` when the element's own code list
    /// leaves the column's `where` codes out, which records it as unwritten.
    fn occurrence_of(
        &mut self,
        index: usize,
        table: &TableDef,
        write: &ColumnWrite<'_>,
    ) -> Result<Option<(usize, Codes)>, Refused> {
        let reader = write.read.loop_id;
        if let OccurrencePlace::Named(occurrence) = write.read.place {
            return Ok(Some((occurrence.index(), Codes::new())));
        }
        match self.resolve(reader, write.segment, write.conditions) {
            Resolved::One(at, codes) => Ok(Some((at, codes))),
            Resolved::Nothing(excluded) => {
                self.unwritten.push(Unwritten {
                    table: index,
                    column: write.column,
                    place: excluded.place,
                    code: excluded.code,
                    codes: excluded.codes,
                });
                Ok(None)
            }
            Resolved::Candidates(candidates) => Err(Box::new(Refusal::AmbiguousSource {
                table: table.name.clone(),
                column: Some(write.name.to_string()),
                loop_name: self.spec.loop_name(reader).to_string(),
                source: render_source(write.segment, write.conditions),
                candidates,
            })),
        }
    }

    /// Refuses a column that reads the segment an envelope loop's `control`
    /// writes.
    fn not_the_envelope(
        &self,
        table: &TableDef,
        write: &ColumnWrite<'_>,
        at: usize,
    ) -> Result<(), Refused> {
        let def = self.spec.get(write.read.loop_id);
        if def.control.is_none() || def.trigger_occurrence() != Some(at) {
            return Ok(());
        }
        Err(Box::new(Refusal::EnvelopeColumn {
            table: table.name.clone(),
            column: write.name.to_string(),
            loop_name: def.name.clone(),
            occurrence: def
                .occurrences
                .get(at)
                .map(|o| o.name.clone())
                .unwrap_or_default(),
        }))
    }

    /// A table with one row per element group of a segment in its anchor loops.
    fn repeat_table(&mut self, index: usize, table: &TableDef) {
        let Some(segment) = &table.segment else {
            return;
        };
        let from = table.repeat.map_or(1, |repeat| repeat.from);
        let elements: Vec<ElementPlan> = self
            .spec
            .places(index)
            .iter()
            .enumerate()
            .filter_map(|(column, place)| written_element(column, place.target, from))
            .collect();
        for &anchor in &table.loops {
            self.repeats.push((index, anchor));
            let def = self.spec.get(anchor);
            match landing(def, def.place_of(segment, &[]), segment) {
                Ok(at) => self.repeat_segment(index, anchor, at, elements.clone()),
                Err(candidates) => self.refusals.push(Refusal::AmbiguousSource {
                    table: table.name.clone(),
                    column: None,
                    loop_name: def.name.clone(),
                    source: render_source(segment, &[]),
                    candidates,
                }),
            }
        }
    }

    /// Adds the segment of table `index`'s rows to occurrence `at` of its
    /// anchor loop, with the qualifier the occurrence fixes.
    fn repeat_segment(
        &mut self,
        index: usize,
        anchor: LoopId,
        at: usize,
        elements: Vec<ElementPlan>,
    ) {
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

/// The element a column of a table anchored on a segment is written in; the
/// segment's index is written nowhere. `from` is the 1-based element a
/// repeat's first group starts at.
fn written_element(column: usize, target: Target, from: usize) -> Option<ElementPlan> {
    match target {
        Target::Element { element, component } => Some(ElementPlan {
            element,
            component,
            value: ValueSource::Column { column },
        }),
        Target::Group { offset, component } => Some(ElementPlan {
            element: from.saturating_add(offset),
            component,
            value: ValueSource::Group { column, offset },
        }),
        Target::Index => None,
    }
}

/// A column of a loop table, as it is written: where it reads, in which
/// segment and element.
struct ColumnWrite<'s> {
    column: usize,
    name: &'s str,
    read: &'s LoopRead,
    relation: Relation,
    segment: &'s [u8],
    conditions: &'s [(usize, Vec<u8>)],
    pick: Pick,
    element: usize,
    component: Option<usize>,
}

impl<'s> ColumnWrite<'s> {
    /// The column at `column`, when it is an element of a loop table with a
    /// loop to read; segment indexes write nothing, and loading refuses a
    /// named occurrence a reading loop lacks, so a column with no read is
    /// not an element column.
    fn of(
        column: usize,
        (name, source): &'s (String, ColumnSource),
        place: &'s ColumnPlace,
    ) -> Option<ColumnWrite<'s>> {
        let ColumnSource::Element {
            segment,
            conditions,
            pick,
            element,
            component,
            ..
        } = source
        else {
            return None;
        };
        let Reading::Watch { relation, reads } = &place.reading else {
            return None;
        };
        Some(ColumnWrite {
            column,
            name,
            read: reads.first()?,
            relation: *relation,
            segment,
            conditions,
            pick: *pick,
            element: *element,
            component: *component,
        })
    }
}

/// How the loop a column reads gets its instances: a column that names no
/// loop gives none; one at or below the anchor gives them per row; one above
/// it, per group of rows.
pub(super) fn instances(relation: Relation, table: usize) -> Option<Instances> {
    match relation {
        Relation::Anchor => None,
        Relation::Inside => Some(Instances::Inside { table }),
        Relation::Above { .. } => Some(Instances::Groups { table }),
    }
}
