//! Where written loops and repeat segments land: the loops written around
//! the written ones, the refusal of repeat segments with no instance to go
//! in, and the position order of every loop's segments.

use super::build::Builder;
use super::plan::Instances;
use super::refusal::Refusal;

impl Builder<'_> {
    /// Every loop above a written loop is written around it.
    pub(super) fn implied(&mut self) {
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

    /// Refuses a table anchored on a segment of a loop that nothing gives
    /// instances to: its segments would have no instance to go in.
    pub(super) fn repeats_placed(&mut self) {
        for (table, anchor) in std::mem::take(&mut self.repeats) {
            let absent = self
                .loops
                .get(anchor.index())
                .is_none_or(|plan| plan.instances == Instances::Absent);
            if !absent {
                continue;
            }
            let segment = self
                .spec
                .tables()
                .get(table)
                .and_then(|def| def.segment.as_deref())
                .map(|segment| format!("{:?}", String::from_utf8_lossy(segment)))
                .unwrap_or_default();
            self.refusals.push(Refusal::RepeatWithoutInstances {
                table: self.table_name(table),
                loop_name: self.spec.loop_name(anchor).to_string(),
                segment,
            });
        }
    }

    pub(super) fn sort(&mut self) {
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
