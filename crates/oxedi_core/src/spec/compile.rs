//! Compilation of the `tables` section into table definitions.

use std::collections::{BTreeMap, BTreeSet};

use super::columns::{check_held, compile_column};
use super::error::SpecError;
use super::phi::compile_phi;
use super::table_error::{AnchorChains, TableDefError};
use super::{RepeatModel, TableModel};
use crate::message::render_key;
use crate::spec::{LoopId, ROW_COLUMN, Repeat, SEGMENT_COLUMN, Spec, TableDef};

/// Compiles the `tables` section against the loops of `spec`, then links
/// every table to the tables above it.
pub(super) fn compile_tables(
    spec: &Spec,
    defs: &BTreeMap<String, TableModel>,
) -> Result<Vec<TableDef>, SpecError> {
    let mut tables = defs
        .iter()
        .map(|(name, def)| compile_table(spec, name, def))
        .collect::<Result<Vec<_>, _>>()?;
    link_tables(spec, &mut tables)?;
    Ok(tables)
}

fn compile_table(spec: &Spec, name: &str, def: &TableModel) -> Result<TableDef, SpecError> {
    let mut build = TableBuild::new(spec, name, def);
    TABLE_STEPS.iter().try_for_each(|step| step(&mut build))?;
    Ok(build.table)
}

/// A table while it compiles: its inputs, and the definition the steps fill
/// in order.
struct TableBuild<'a> {
    spec: &'a Spec,
    name: &'a str,
    def: &'a TableModel,
    /// The table so far; each step sets the fields it owns.
    table: TableDef,
}

type TableStep = fn(&mut TableBuild<'_>) -> Result<(), SpecError>;

/// The table compile steps, in the order they run; the first error wins.
const TABLE_STEPS: [TableStep; 8] = [
    check_name,
    resolve_anchors,
    resolve_reference,
    resolve_segment,
    compile_repeat,
    check_anchors,
    compile_columns,
    compile_table_phi,
];

impl<'a> TableBuild<'a> {
    fn new(spec: &'a Spec, name: &'a str, def: &'a TableModel) -> Self {
        let table = TableDef {
            name: name.to_string(),
            reference: name.to_string(),
            loops: Vec::with_capacity(def.loops.len()),
            segment: None,
            repeat: None,
            columns: Vec::with_capacity(def.columns.len()),
            parent: None,
            ancestors: Vec::new(),
            phi: Vec::new(),
            phi_seed: None,
        };
        TableBuild {
            spec,
            name,
            def,
            table,
        }
    }

    fn fail(&self, reason: TableDefError) -> SpecError {
        SpecError::BadTable {
            table: self.name.to_string(),
            column: None,
            reason,
        }
    }
}

impl Repeat {
    /// Checks a table's `repeat`: it needs the table's `segment`, a 1-based
    /// `from` and a non-zero `step`.
    fn compile(raw: &RepeatModel, segment: Option<&[u8]>) -> Result<Repeat, TableDefError> {
        if segment.is_none() {
            return Err(TableDefError::RepeatWithoutSegment);
        }
        if raw.from == 0 {
            return Err(TableDefError::ZeroPosition { key: "repeat.from" });
        }
        if raw.step == 0 {
            return Err(TableDefError::ZeroStep);
        }
        Ok(Repeat {
            from: raw.from,
            step: raw.step,
        })
    }
}

fn check_name(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    if build.name.is_empty() {
        return Err(build.fail(TableDefError::EmptyName { what: "table name" }));
    }
    Ok(())
}

/// Resolves the anchor loops; a loop named twice is refused through a set of
/// the ids seen so far.
fn resolve_anchors(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    if build.def.loops.is_empty() {
        return Err(build.fail(TableDefError::NoLoops));
    }
    let mut seen = BTreeSet::new();
    for loop_name in &build.def.loops {
        let id = build.spec.loop_id(loop_name).ok_or_else(|| {
            build.fail(TableDefError::UnknownLoop {
                name: loop_name.clone(),
            })
        })?;
        if !seen.insert(id) {
            return Err(build.fail(TableDefError::DuplicateLoop {
                loop_name: loop_name.clone(),
            }));
        }
        build.table.loops.push(id);
    }
    Ok(())
}

fn resolve_reference(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    let reference = build
        .def
        .reference
        .clone()
        .unwrap_or_else(|| build.name.to_string());
    if reference.is_empty() {
        return Err(build.fail(TableDefError::EmptyName { what: "ref" }));
    }
    if reference == ROW_COLUMN || reference == SEGMENT_COLUMN {
        return Err(build.fail(TableDefError::ReservedName { name: reference }));
    }
    build.table.reference = reference;
    Ok(())
}

fn resolve_segment(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    build.table.segment = match build.def.segment.as_deref() {
        None => None,
        Some("") => {
            return Err(SpecError::EmptySegmentId {
                loop_name: None,
                key: format!("tables.{}.segment", render_key(build.name)),
            });
        }
        Some(id) => Some(id.as_bytes().to_vec()),
    };
    Ok(())
}

fn compile_repeat(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    let segment = build.table.segment.as_deref();
    build.table.repeat = build
        .def
        .repeat
        .as_ref()
        .map(|raw| Repeat::compile(raw, segment))
        .transpose()
        .map_err(|reason| build.fail(reason))?;
    Ok(())
}

/// A table on a segment needs every anchor to hold it; one without a segment
/// needs anchors none of which sits inside another.
fn check_anchors(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    let table = &build.table;
    let Some(anchor) = &table.segment else {
        return check_nested_anchors(build);
    };
    check_held(build.spec, &table.loops, anchor).map_err(|reason| build.fail(reason))
}

/// Reports the first anchor (in order) that is an ancestor of another anchor,
/// with the first anchor below it. One pass records, for every ancestor, the
/// first anchor under it; a lookup per anchor then finds the pair.
fn check_nested_anchors(build: &TableBuild<'_>) -> Result<(), SpecError> {
    let (spec, loops) = (build.spec, &build.table.loops);
    let mut below = BTreeMap::new();
    for (ancestor, inner) in loops
        .iter()
        .flat_map(|&inner| spec.ancestors(inner).into_iter().map(move |a| (a, inner)))
    {
        below.entry(ancestor).or_insert(inner);
    }
    match loops
        .iter()
        .find_map(|outer| below.get(outer).map(|inner| (*outer, *inner)))
    {
        Some((outer, inner)) => Err(build.fail(TableDefError::NestedAnchors {
            outer: spec.loop_name(outer).to_string(),
            inner: spec.loop_name(inner).to_string(),
        })),
        None => Ok(()),
    }
}

fn compile_columns(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    let TableBuild {
        spec,
        name,
        def,
        table,
    } = build;
    for (column, raw) in &def.columns {
        let source = compile_column(
            spec,
            name,
            column,
            raw,
            &table.loops,
            table.segment.as_deref(),
            table.repeat,
        )?;
        table.columns.push((column.clone(), source));
    }
    Ok(())
}

fn compile_table_phi(build: &mut TableBuild<'_>) -> Result<(), SpecError> {
    compile_phi(
        build.spec,
        &mut build.table,
        build.def.columns.values().map(|column| column.phi),
        build.def.phi_seed.as_deref(),
    )
}

/// Rejects a loop anchoring two tables without `segment` and a `ref` used
/// twice, then gives each table its chain of tables above: the tables
/// anchored on the loops above each anchor (from the anchor loop itself for
/// a table anchored on a segment, whose rows live inside that loop). Chains
/// run outermost first, and every anchor's chain must begin the longest one.
fn link_tables(spec: &Spec, tables: &mut [TableDef]) -> Result<(), SpecError> {
    let bad = |table: &TableDef, column: Option<&str>, reason| SpecError::BadTable {
        table: table.name.clone(),
        column: column.map(str::to_string),
        reason,
    };
    let mut anchored: Vec<Option<usize>> = vec![None; spec.loops().len()];
    for (index, table) in tables.iter().enumerate() {
        if table.segment.is_some() {
            continue;
        }
        for &id in &table.loops {
            if let Some(other) = anchored[id.index()] {
                return Err(bad(
                    table,
                    None,
                    TableDefError::SharedAnchor {
                        loop_name: spec.loop_name(id).to_string(),
                        other: tables[other].name.clone(),
                    },
                ));
            }
            anchored[id.index()] = Some(index);
        }
    }
    for (index, table) in tables.iter().enumerate() {
        if let Some(first) = tables[..index]
            .iter()
            .find(|other| other.reference == table.reference)
        {
            return Err(bad(
                table,
                None,
                TableDefError::RefTaken {
                    name: table.reference.clone(),
                    table: first.name.clone(),
                },
            ));
        }
    }
    for index in 0..tables.len() {
        let table = &tables[index];
        let mut longest: Option<(LoopId, Vec<usize>)> = None;
        let mut chains = Vec::with_capacity(table.loops.len());
        for &anchor in &table.loops {
            let mut chain = Vec::new();
            let mut current = if table.segment.is_some() {
                Some(anchor)
            } else {
                spec.get(anchor).parent
            };
            while let Some(id) = current {
                if let Some(owner) = anchored[id.index()] {
                    chain.push(owner);
                }
                current = spec.get(id).parent;
            }
            chain.reverse();
            if longest
                .as_ref()
                .is_none_or(|(_, best)| chain.len() > best.len())
            {
                longest = Some((anchor, chain.clone()));
            }
            chains.push((anchor, chain));
        }
        let Some((first, ancestors)) = longest else {
            continue;
        };
        for (anchor, chain) in &chains {
            if !ancestors.starts_with(chain) {
                return Err(bad(
                    table,
                    None,
                    TableDefError::UnrelatedAnchors {
                        first: spec.loop_name(first).to_string(),
                        second: spec.loop_name(*anchor).to_string(),
                        chains: Box::new(AnchorChains {
                            first: ancestors
                                .iter()
                                .map(|&above| tables[above].name.clone())
                                .collect(),
                            second: chain
                                .iter()
                                .map(|&above| tables[above].name.clone())
                                .collect(),
                        }),
                    },
                ));
            }
        }
        for (column, _) in &table.columns {
            if ancestors
                .iter()
                .any(|&above| tables[above].reference == *column)
            {
                return Err(bad(
                    table,
                    Some(column),
                    TableDefError::ReservedName {
                        name: column.clone(),
                    },
                ));
            }
        }
        let table = &mut tables[index];
        table.parent = ancestors.last().copied();
        table.ancestors = ancestors;
    }
    Ok(())
}
