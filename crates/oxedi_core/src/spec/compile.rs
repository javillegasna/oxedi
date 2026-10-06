//! Compilation of the `tables` section into table definitions.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::Spec;
use super::columns::{check_held, compile_column};
use super::error::SpecError;
use super::loops::LoopId;
use super::raw::{RawColumn, RawTable};
use super::render::render_key;
use super::segments::{ROW_COLUMN, SEGMENT_COLUMN};
use super::shape::section;
use super::table_error::{AnchorChains, TableDefError};
use super::tables::{Repeat, TableDef};

/// Compiles the `tables` section against the loops of `spec`, then links
/// every table to the tables above it.
pub(super) fn compile_tables(
    spec: &Spec,
    raw: BTreeMap<&str, &Value>,
) -> Result<Vec<TableDef>, SpecError> {
    let mut tables = Vec::with_capacity(raw.len());
    for (name, value) in raw {
        let def = RawTable::deserialize(value).map_err(|e| SpecError::TableSchema {
            table: name.to_string(),
            column: None,
            source: e,
        })?;
        tables.push(compile_table(spec, name, &def, value)?);
    }
    link_tables(spec, &mut tables)?;
    Ok(tables)
}

fn compile_table(
    spec: &Spec,
    name: &str,
    def: &RawTable,
    value: &Value,
) -> Result<TableDef, SpecError> {
    let fail = |reason| SpecError::BadTable {
        table: name.to_string(),
        column: None,
        reason,
    };
    if name.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "table name" }));
    }
    if def.loops.is_empty() {
        return Err(fail(TableDefError::NoLoops));
    }
    let mut loops = Vec::with_capacity(def.loops.len());
    for loop_name in &def.loops {
        let id = spec.loop_id(loop_name).ok_or_else(|| {
            fail(TableDefError::UnknownLoop {
                name: loop_name.clone(),
            })
        })?;
        if loops.contains(&id) {
            return Err(fail(TableDefError::DuplicateLoop {
                loop_name: loop_name.clone(),
            }));
        }
        loops.push(id);
    }
    let reference = def.reference.clone().unwrap_or_else(|| name.to_string());
    if reference.is_empty() {
        return Err(fail(TableDefError::EmptyName { what: "ref" }));
    }
    if reference == ROW_COLUMN || reference == SEGMENT_COLUMN {
        return Err(fail(TableDefError::ReservedName { name: reference }));
    }
    let segment = match def.segment.as_deref() {
        None => None,
        Some("") => {
            return Err(SpecError::EmptySegmentId {
                loop_name: None,
                key: format!("tables.{}.segment", render_key(name)),
            });
        }
        Some(id) => Some(id.as_bytes().to_vec()),
    };
    let repeat = match &def.repeat {
        None => None,
        Some(_) if segment.is_none() => return Err(fail(TableDefError::RepeatWithoutSegment)),
        Some(raw) if raw.from == 0 => {
            return Err(fail(TableDefError::ZeroPosition { key: "repeat.from" }));
        }
        Some(raw) if raw.step == 0 => return Err(fail(TableDefError::ZeroStep)),
        Some(raw) => Some(Repeat {
            from: raw.from,
            step: raw.step,
        }),
    };
    if let Some(anchor) = &segment {
        check_held(spec, &loops, anchor).map_err(fail)?;
    }
    if segment.is_none() {
        for &a in &loops {
            for &b in &loops {
                if spec.ancestors(b).contains(&a) {
                    return Err(fail(TableDefError::NestedAnchors {
                        outer: spec.loop_name(a).to_string(),
                        inner: spec.loop_name(b).to_string(),
                    }));
                }
            }
        }
    }
    let mut columns = Vec::with_capacity(def.columns.len());
    for (column, value) in section(value, "columns") {
        let raw = RawColumn::deserialize(value).map_err(|e| SpecError::TableSchema {
            table: name.to_string(),
            column: Some(column.to_string()),
            source: e,
        })?;
        let source = compile_column(spec, name, column, &raw, &loops, segment.as_deref(), repeat)?;
        columns.push((column.to_string(), source));
    }
    Ok(TableDef {
        name: name.to_string(),
        reference,
        loops,
        segment,
        repeat,
        columns,
        parent: None,
        ancestors: Vec::new(),
    })
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
