//! The registry and the structs a declaration emits read as pinned here, so
//! moving an object into a declaration changes neither a key, its order, its
//! field, its required flag nor its serde behaviour.

use std::collections::BTreeMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde_json::{from_value, json};

use crate::spec::raw::{RawBalance, RawControl, RawTerm};
use crate::spec::sections::ROOT;
use crate::spec::shape::{Field, Leaf, ObjectSchema};
use crate::spec::{
    BalanceModel, ControlModel, QualifierModel, RepeatModel, SummaryColumnModel, SummaryFieldModel,
    SummaryRowModel, SummaryTableModel, TermModel, VersionModel,
};

/// The registry objects in the order a depth-first walk from the root first
/// visits them, each with its path, and the path of each by its address.
#[derive(Default)]
struct Names {
    order: Vec<(String, &'static ObjectSchema)>,
    by_address: BTreeMap<usize, String>,
}

fn address(schema: &'static ObjectSchema) -> usize {
    std::ptr::from_ref(schema) as usize
}

impl Names {
    // `base` is the path the field sits at; a map member adds `.*` and a list
    // item `[]`, and an object's keys hang under its path (the root's under none).
    fn visit(&mut self, field: &'static Field, base: String) {
        match field {
            Field::Object(schema) => {
                if self.by_address.contains_key(&address(schema)) {
                    return;
                }
                let prefix = if self.order.is_empty() {
                    String::new()
                } else {
                    format!("{base}.")
                };
                self.by_address.insert(address(schema), base.clone());
                self.order.push((base, schema));
                for (key, inner) in schema.keys {
                    self.visit(inner, format!("{prefix}{key}"));
                }
            }
            Field::Map(item) => self.visit(item, format!("{base}.*")),
            Field::List { item, .. } => self.visit(item, format!("{base}[]")),
            Field::Leaf { .. } => {}
        }
    }

    fn render(&self, field: &Field) -> String {
        match field {
            Field::Leaf { kind, nullable } => {
                let word = match kind {
                    Leaf::Text => "text",
                    Leaf::Flag => "flag",
                    Leaf::Count => "count",
                    Leaf::Byte => "byte",
                    Leaf::Pick => "pick",
                };
                format!("{word}{}", if *nullable { "?" } else { "" })
            }
            Field::List { item, .. } => format!("[{}]", self.render(item)),
            Field::Map(item) => format!("{{{}}}", self.render(item)),
            Field::Object(schema) => format!("@{}", self.by_address[&address(schema)]),
        }
    }

    fn line(&self, path: &str, schema: &ObjectSchema) -> String {
        let keys = schema
            .keys
            .iter()
            .map(|(key, field)| {
                let mark = if schema.required.contains(key) {
                    "!"
                } else {
                    ""
                };
                format!("{key}{mark}={}", self.render(field))
            })
            .collect::<Vec<_>>();
        format!("{path}: {}", keys.join(", "))
    }
}

static TOP: Field = Field::Object(&ROOT);

/// One line per registry object: its keys in order, each with its field and a
/// `!` when required.
fn registry_text() -> String {
    let mut names = Names::default();
    names.visit(&TOP, "spec".to_string());
    names
        .order
        .iter()
        .map(|(path, schema)| names.line(path, schema))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `"{expecting}: {keys}"`: the struct's `expecting` text and its keys in
/// field order, both as serde reports them.
fn pinned<T: DeserializeOwned + Debug>() -> String {
    let wrong_type = from_value::<T>(json!(1)).unwrap_err().to_string();
    let expecting = wrong_type
        .split_once("expected ")
        .map_or("", |(_, tail)| tail);
    let unknown = from_value::<T>(json!({"?": 0})).unwrap_err().to_string();
    let keys = unknown
        .split('`')
        .skip(1)
        .step_by(2)
        .skip(1)
        .collect::<Vec<_>>();
    format!("{expecting}: {}", keys.join(" "))
}

/// A struct's name and how its pin is read.
type Pin = (&'static str, fn() -> String);

static STRUCTS: &[Pin] = &[
    ("ControlModel", pinned::<ControlModel>),
    ("RawControl", pinned::<RawControl>),
    ("VersionModel", pinned::<VersionModel>),
    ("QualifierModel", pinned::<QualifierModel>),
    ("RepeatModel", pinned::<RepeatModel>),
    ("BalanceModel", pinned::<BalanceModel>),
    ("RawBalance", pinned::<RawBalance>),
    ("TermModel", pinned::<TermModel>),
    ("RawTerm", pinned::<RawTerm>),
    ("SummaryTableModel", pinned::<SummaryTableModel>),
    ("SummaryColumnModel", pinned::<SummaryColumnModel>),
    ("SummaryRowModel", pinned::<SummaryRowModel>),
    ("SummaryFieldModel", pinned::<SummaryFieldModel>),
];

const REGISTRY: &str = "\
spec: name!=text, version=@version, loops!={@loops.*}, tables={@tables.*}, balancing={@balancing.*}, segments={@segments.*}, summaries={@summaries.*}, default_table=text
version: segment!=text, element!=count, values!=[text]
loops.*: parent=text?, end=text?, max=count?, usage=text?, occurrences={@loops.*.occurrences.*}, trigger!=@loops.*.trigger, control=@loops.*.control
loops.*.occurrences.*: segment!=text, pos!=count, usage=text?, max=count?, qualifier=@loops.*.occurrences.*.qualifier, codes={[text]}
loops.*.occurrences.*.qualifier: element!=count, component=count?, codes!=[text]
loops.*.trigger: segment!=text, where={text}
loops.*.control: opener_element!=count, closer_element!=count, count_element!=count, count!=text
tables.*: loops!=[text], ref=text?, segment=text?, phi_seed=text?, repeat=@tables.*.repeat, columns={@tables.*.columns.*}
tables.*.repeat: from!=count, step!=count
tables.*.columns.*: loop=text?, segment=text?, where={text}, occurrence=text?, pick=pick?, element=count?, component=count?, group_element=count?, segment_index=flag, phi=text?
balancing.*: per!=text, target!=[@balancing.*.target[]], sum!=[@balancing.*.target[]]
balancing.*.target[]: loop=text?, occurrence!=text, component=count?, sign=text?, elements!=[count]
segments.*: elements={@segments.*.elements.*}
segments.*.elements.*: codes=[text], name!=text, type!=text, required=flag, min=count?, max=count?, scale=byte?, composite={@segments.*.elements.*}
summaries.*: count=@summaries.*.count, distinct=@summaries.*.distinct, sum=@summaries.*.distinct, row=@summaries.*.row
summaries.*.count: table!=text
summaries.*.distinct: table!=text, column!=text
summaries.*.row: table!=text, fields!=[@summaries.*.row.fields[]]
summaries.*.row.fields[]: name!=text, column!=text";

const PINNED: &str = "\
ControlModel a control object: opener_element closer_element count_element count
RawControl a control object: opener_element closer_element count_element count
VersionModel a version object: segment element values
QualifierModel a qualifier object: element component codes
RepeatModel a repeat object: from step
BalanceModel a balancing rule object: per target sum
RawBalance a balancing rule object: per target sum
TermModel a balancing value object: loop occurrence component sign elements
RawTerm a balancing value object: loop occurrence component sign elements
SummaryTableModel a summary operation object: table
SummaryColumnModel a summary operation object: table column
SummaryRowModel a summary operation object: table fields
SummaryFieldModel a summary field object: name column";

#[test]
fn the_registry_reads_as_pinned() {
    assert_eq!(registry_text(), REGISTRY);
}

#[test]
fn every_declared_struct_pins_its_expecting_and_its_keys() {
    let text = STRUCTS
        .iter()
        .map(|(name, pinned)| format!("{name} {}", pinned()))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(text, PINNED);
}
