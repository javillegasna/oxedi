//! The viewer's picture of one file: segments with their byte ranges and
//! owning loop instance, the loop tree, the projected tables as text and
//! the diagnostics with their location.

use oxedi_core::api::{ReadOptions, read_tree};
use oxedi_core::problem::{Detail, Place, Problem, ProblemRecord};
use oxedi_core::{Cell, DateMode, Diagnostic, Document, LoopTree, Mask, NodeId, Spec, Tables};
use serde::Serialize;
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// One file as the viewer shows it.
#[derive(Debug, Serialize)]
pub struct View {
    /// The declared version the chosen spec covers, e.g. `005010X221A1`.
    pub spec_version: Option<String>,
    /// The file's separators, as text.
    pub delimiters: DelimitersView,
    /// Every segment, in file order.
    pub segments: Vec<SegmentView>,
    /// Loop instances; index 0 is the root.
    pub nodes: Vec<NodeView>,
    /// The spec's tables, ordered by name.
    pub tables: Vec<TableView>,
    /// Every diagnostic, in the order the core reports them.
    pub diagnostics: Vec<DiagnosticView>,
}

/// The separators of a file. Each is one byte written as the `char` with
/// that code point (Latin-1), so `charCodeAt(0)` in JS gives the byte back;
/// encoding the string as UTF-8 does not for bytes from 0x80 up.
#[derive(Debug, Serialize)]
pub struct DelimitersView {
    /// Separates the elements of a segment.
    pub element: String,
    /// Separates the components of a composite element.
    pub component: String,
    /// Ends a segment.
    pub segment: String,
    /// Separates repeated values of an element; `None` when the file declares none.
    pub repetition: Option<String>,
}

/// One segment: its id, where its raw bytes live and the loop instance that holds it.
#[derive(Debug, Serialize)]
pub struct SegmentView {
    /// The segment id as text; empty for a segment with no content.
    pub id: String,
    /// First byte of the segment's raw range (trivia, body, terminator).
    pub start: usize,
    /// One past its last byte.
    pub end: usize,
    /// Index into `View::nodes` of the innermost loop instance open when
    /// the segment was read, including a segment with no content; 0 (the
    /// root) outside every loop.
    pub node: usize,
}

/// One loop instance.
#[derive(Debug, Serialize)]
pub struct NodeView {
    /// The loop's name as the spec writes it; `None` for the root.
    pub loop_name: Option<String>,
    /// 1 for the first instance of its loop in the file, 2 for the second…; 0 for the root.
    pub ordinal: usize,
    /// `true` when no segment of its own opened it.
    pub implicit: bool,
    /// Index into `View::nodes` of the enclosing instance; `None` for the root.
    pub parent: Option<usize>,
    /// Indices into `View::nodes` of the nested instances, in file order.
    pub children: Vec<usize>,
}

/// One projected table, column by column.
#[derive(Debug, Serialize)]
pub struct TableView {
    /// The table's name in the spec.
    pub name: String,
    /// Number of rows; every column holds this many values.
    pub rows: usize,
    /// The columns, in the spec's order.
    pub columns: Vec<ColumnView>,
}

/// One column: each row as text, `None` for a null.
#[derive(Debug, Serialize)]
pub struct ColumnView {
    /// The column's name in the spec.
    pub name: String,
    /// Each row as the core renders it: decimals in fixed point, dates as
    /// `YYYY-MM-DD`, times as `HH:MM:SS`, text with invalid UTF-8 replaced.
    pub values: Vec<Option<String>>,
}

/// One diagnostic with its full text and where it points.
#[derive(Debug, Serialize)]
pub struct DiagnosticView {
    /// The diagnostic's full text: rule, location and offending datum.
    pub text: String,
    /// `SNIP 1`, `SNIP 2` or `SNIP 3`.
    pub level: String,
    /// 0-based index into `View::segments` of the segment at fault; `None`
    /// at the end of the stream.
    pub segment: Option<usize>,
    /// 1-based element position, when the diagnostic has one.
    pub element: Option<usize>,
    /// 1-based component position, when the diagnostic has one.
    pub component: Option<usize>,
    /// Open loops, outermost first, as `(name, ordinal)`.
    pub path: Vec<(String, usize)>,
}

/// Parses `bytes` with the built-in spec of their declared version; the
/// error is the core's message for bytes that are not an X12 interchange.
pub fn view(bytes: &[u8]) -> Result<View, String> {
    view_with(bytes, None)
}

/// [`view`] with the columns the spec flags as patient data masked by
/// `mask_key` (dates reduced to the year) when a key is given; a key the
/// core refuses is returned as its message.
pub fn view_masked(bytes: &[u8], mask_key: Option<&[u8]>) -> Result<View, String> {
    let mask = mask_key
        .map(|key| Mask::new(key, DateMode::Year).map_err(|err| err.to_string()))
        .transpose()?;
    view_with(bytes, mask.as_ref())
}

fn view_with(bytes: &[u8], mask: Option<&Mask>) -> Result<View, String> {
    let options = ReadOptions {
        mask,
        ..ReadOptions::<Arc<Spec>>::default()
    };
    let (reading, tree) = read_tree(bytes, options).map_err(|err| err.to_string())?;
    let spec = reading.spec.spec();
    let document = &reading.document;
    let delimiters = document.delimiters();
    let text = |byte: u8| char::from(byte).to_string();
    Ok(View {
        spec_version: spec
            .version()
            .and_then(|version| version.values.first())
            .map(|value| String::from_utf8_lossy(value).into_owned()),
        delimiters: DelimitersView {
            element: text(delimiters.element),
            component: text(delimiters.component),
            segment: text(delimiters.segment),
            repetition: delimiters.repetition.map(text),
        },
        segments: segments(document, &tree),
        nodes: nodes(spec, &tree),
        tables: tables_view(&reading.tables),
        diagnostics: reading
            .diagnostics
            .iter()
            .map(diagnostic)
            .collect::<Result<_, _>>()?,
    })
}

/// [`view_masked`] as the JSON the page reads.
pub fn to_json(bytes: &[u8], mask_key: Option<&[u8]>) -> Result<String, String> {
    serde_json::to_string(&view_masked(bytes, mask_key)?).map_err(|err| err.to_string())
}

fn segments(document: &Document<'_>, tree: &LoopTree) -> Vec<SegmentView> {
    document
        .spans()
        .enumerate()
        .map(|(index, span)| SegmentView {
            id: String::from_utf8_lossy(document.segment_id(index).unwrap_or_default())
                .into_owned(),
            start: span.raw.start,
            end: span.raw.end,
            node: tree.owner(index).index(),
        })
        .collect()
}

fn nodes(spec: &Spec, tree: &LoopTree) -> Vec<NodeView> {
    tree.nodes()
        .iter()
        .map(|node| NodeView {
            loop_name: node.loop_id.map(|id| spec.loop_name(id).to_string()),
            // 0 is the root's documented value: it has no ordinal.
            ordinal: node.ordinal.unwrap_or(0),
            implicit: node.implicit,
            parent: node.parent.map(NodeId::index),
            children: node.children.iter().map(|child| child.index()).collect(),
        })
        .collect()
}

fn tables_view(tables: &Tables) -> Vec<TableView> {
    tables
        .iter()
        .map(|table| TableView {
            name: table.name().to_string(),
            rows: table.len(),
            columns: table
                .columns()
                .iter()
                .map(|(name, data)| ColumnView {
                    name: name.clone(),
                    values: (0..data.len())
                        .map(|row| match data.get(row) {
                            Some(Cell::Null) | None => None,
                            Some(_) => data.render(row),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

/// One diagnostic as the page reads it, from its record: the file place and
/// the reading detail. Any other place or detail is not a diagnostic of a
/// file.
fn diagnostic(diagnostic: &Diagnostic) -> Result<DiagnosticView, String> {
    let ProblemRecord {
        kind,
        place,
        text,
        detail,
    } = diagnostic.record();
    let (
        Place::File {
            segment,
            element,
            component,
            path,
        },
        Detail::Diagnostic { level, .. },
    ) = (place, detail)
    else {
        return Err(format!(
            "internal error: the problem {kind} is not a diagnostic of a file"
        ));
    };
    Ok(DiagnosticView {
        text,
        level: level.to_string(),
        segment,
        element,
        component,
        path: path
            .iter()
            .map(|loop_ref| (loop_ref.name.clone(), loop_ref.ordinal))
            .collect(),
    })
}
