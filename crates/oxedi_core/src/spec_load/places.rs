//! The place of every declared column: its type, what of a segment it reads
//! and which loops and occurrences it reads.

use crate::spec::{
    ColumnPlace, ColumnSource, LoopId, LoopRead, OccurrencePlace, Reading, Relation, Spec,
    TableDef, Target, column_type,
};

/// The places of the declared columns of every table of `spec`, per table in
/// column order.
pub(super) fn place_columns(spec: &Spec) -> Vec<Vec<ColumnPlace>> {
    spec.tables()
        .iter()
        .map(|def| {
            let mut carried = 0;
            def.columns
                .iter()
                .map(|(_, source)| ColumnPlace {
                    kind: column_type(spec, def, source),
                    target: target_of(source),
                    reading: reading(spec, def, source, &mut carried),
                })
                .collect()
        })
        .collect()
}

fn target_of(source: &ColumnSource) -> Target {
    match source {
        ColumnSource::Element {
            element, component, ..
        } => Target::Element {
            element: *element,
            component: *component,
        },
        ColumnSource::GroupElement { offset, component } => Target::Group {
            offset: *offset,
            component: *component,
        },
        ColumnSource::SegmentIndex { .. } => Target::Index,
    }
}

/// Which loops and occurrences `source` reads. `carried` counts the table's
/// carried values so far; a column that reads above the anchor takes the next.
fn reading(spec: &Spec, def: &TableDef, source: &ColumnSource, carried: &mut usize) -> Reading {
    let (ColumnSource::Element {
        loop_id,
        segment,
        conditions,
        occurrence,
        ..
    }
    | ColumnSource::SegmentIndex {
        loop_id,
        segment,
        conditions,
        occurrence,
        ..
    }) = source
    else {
        return Reading::Anchor;
    };
    if def.segment.is_some() {
        return Reading::Anchor;
    }
    let readers = loop_id.map_or_else(|| def.loops.clone(), |id| vec![id]);
    let reads = readers
        .into_iter()
        .filter_map(|id| {
            // A loop that does not declare the named occurrence gives no read;
            // loading refuses a spec whose named occurrence a loop lacks, which
            // the resets of an `Above` column rely on.
            let place = match occurrence {
                Some(name) => OccurrencePlace::Named(spec.get(id).occurrence_named(name)?),
                None => spec.get(id).place_of(segment, conditions),
            };
            Some(LoopRead { loop_id: id, place })
        })
        .collect();
    Reading::Watch {
        relation: relation(spec, def, *loop_id, carried),
        reads,
    }
}

/// How the loop a column names sits against its table's anchor loops.
fn relation(spec: &Spec, def: &TableDef, named: Option<LoopId>, carried: &mut usize) -> Relation {
    let Some(id) = named else {
        return Relation::Anchor;
    };
    let above = def
        .loops
        .iter()
        .any(|&anchor| spec.ancestors(anchor).contains(&id));
    if !above {
        return Relation::Inside;
    }
    *carried += 1;
    Relation::Above {
        carried: *carried - 1,
    }
}
