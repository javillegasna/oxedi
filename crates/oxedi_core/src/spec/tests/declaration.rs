//! The registry reads as pinned here, so a key, its order, its field or its
//! required flag does not change unseen, and a model writes no default and no
//! `null`.

use std::collections::BTreeMap;

use serde_json::{from_value, json};

use crate::spec::model::{ROOT, RawColumn};
use crate::spec::shape::{Field, Leaf, ObjectSchema};
use crate::spec::{ColumnModel, LoopModel, SpecModel, TableModel, TriggerModel};

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

#[test]
fn the_registry_reads_as_pinned() {
    assert_eq!(registry_text(), REGISTRY);
}

#[test]
fn a_table_and_its_columns_write_no_default_and_no_null() {
    let model = TableModel {
        loops: vec!["A".to_string()],
        reference: None,
        segment: None,
        repeat: None,
        columns: BTreeMap::from([(
            "c".to_string(),
            ColumnModel {
                loop_name: None,
                segment: None,
                conditions: BTreeMap::new(),
                occurrence: None,
                pick: None,
                element: None,
                component: None,
                group_element: None,
                segment_index: false,
                phi: None,
            },
        )]),
        phi_seed: None,
    };
    let written = json!({"loops": ["A"], "columns": {"c": {}}});
    assert_eq!(serde_json::to_value(&model).unwrap(), written);
    let nulls = json!({
        "loops": ["A"], "ref": null, "segment": null, "phi_seed": null,
        "columns": {"c": {
            "loop": null, "segment": null, "where": {}, "occurrence": null, "pick": null,
            "element": null, "component": null, "group_element": null,
            "segment_index": false, "phi": null
        }}
    });
    assert_eq!(from_value::<TableModel>(nulls.clone()).unwrap(), model);
    let raw = from_value::<RawColumn>(nulls["columns"]["c"].clone()).unwrap();
    assert!(raw.loop_name.is_none(), "RawColumn.loop_name");
    assert!(raw.segment.is_none(), "RawColumn.segment");
    assert!(raw.conditions.is_empty(), "RawColumn.conditions");
    assert!(raw.occurrence.is_none(), "RawColumn.occurrence");
    assert!(raw.pick.is_none(), "RawColumn.pick");
    assert!(raw.element.is_none(), "RawColumn.element");
    assert!(raw.component.is_none(), "RawColumn.component");
    assert!(raw.group_element.is_none(), "RawColumn.group_element");
    assert!(!raw.segment_index, "RawColumn.segment_index");
    assert!(raw.phi.is_none(), "RawColumn.phi");
}

#[test]
fn a_spec_model_writes_no_default_and_no_null() {
    let model = SpecModel {
        name: "n".to_string(),
        version: None,
        loops: BTreeMap::from([(
            "A".to_string(),
            LoopModel {
                parent: None,
                trigger: TriggerModel {
                    segment: "ST".to_string(),
                    conditions: BTreeMap::new(),
                },
                occurrences: BTreeMap::new(),
                max: None,
                usage: Default::default(),
                end: None,
                control: None,
            },
        )]),
        segments: BTreeMap::new(),
        tables: BTreeMap::new(),
        balancing: BTreeMap::new(),
        summaries: BTreeMap::new(),
        default_table: None,
    };
    let written = json!({"name": "n", "loops": {"A": {"trigger": {"segment": "ST"}}}});
    assert_eq!(serde_json::to_value(&model).unwrap(), written);
    let nulls = json!({
        "name": "n", "loops": {"A": {"trigger": {"segment": "ST"}}},
        "segments": {}, "tables": {}, "balancing": {}, "summaries": {}, "default_table": null
    });
    assert_eq!(from_value::<SpecModel>(nulls).unwrap(), model);
}
