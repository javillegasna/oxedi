//! The place of every declared column: its type and what of a segment it reads.

use crate::spec::{ColumnPlace, ColumnSource, Spec, Target, column_type};

/// The places of the declared columns of every table of `spec`, per table in
/// column order.
pub(super) fn place_columns(spec: &Spec) -> Vec<Vec<ColumnPlace>> {
    spec.tables()
        .iter()
        .map(|def| {
            def.columns
                .iter()
                .map(|(_, source)| ColumnPlace {
                    kind: column_type(spec, def, source),
                    target: target_of(source),
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
