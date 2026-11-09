//! The hand-written spec objects, the element and the summary, and the
//! section registry name the same keys, so a key added to one and not the
//! other fails here. The declared objects emit both from one declaration.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, from_value, json, to_value};

use crate::spec::model::ROOT;
use crate::spec::sections::{ELEMENT, SUMMARY};
use crate::spec::shape::{Field, ObjectSchema, check_shape};
use crate::spec::{
    ElementModel, SegmentModel, SpecModel, SummaryColumnModel, SummaryFieldModel, SummaryModel,
    SummaryRowModel, SummaryTableModel,
};
use crate::value::ElementType;

fn one<K: Ord, V>(key: K, value: V) -> BTreeMap<K, V> {
    BTreeMap::from([(key, value)])
}

fn text(word: &str) -> String {
    word.to_string()
}

fn element(composite: BTreeMap<usize, ElementModel>) -> ElementModel {
    ElementModel {
        name: text("amount"),
        kind: ElementType::R { scale: 4 },
        required: true,
        min: Some(1),
        max: Some(9),
        codes: Some(vec![text("A")]),
        composite,
    }
}

// Every field set off its default and every optional one present, written as
// full literals so a field added to a model stops this build until it is set.
fn every_key_model() -> SpecModel {
    let summary_field = SummaryFieldModel {
        name: text("n"),
        column: text("c"),
    };
    let summary_column = SummaryColumnModel {
        table: text("claims"),
        column: text("c"),
    };
    SpecModel {
        name: text("every_key"),
        version: None,
        loops: one(
            text("2100"),
            from_value(json!({"trigger": {"segment": "CLP"}})).unwrap(),
        ),
        segments: one(
            text("CLP"),
            SegmentModel {
                elements: one(1, element(one(1, element(BTreeMap::new())))),
            },
        ),
        tables: BTreeMap::new(),
        balancing: BTreeMap::new(),
        summaries: BTreeMap::from([
            (
                text("count"),
                SummaryModel::Count(SummaryTableModel {
                    table: text("claims"),
                }),
            ),
            (
                text("distinct"),
                SummaryModel::Distinct(summary_column.clone()),
            ),
            (text("sum"), SummaryModel::Sum(summary_column)),
            (
                text("row"),
                SummaryModel::Row(SummaryRowModel {
                    table: text("claims"),
                    fields: vec![summary_field],
                }),
            ),
        ]),
        default_table: None,
    }
}

// No `_` arm: a new `SummaryModel` variant stops this build, and its slot must
// be set by `every_key_model`. The private `type_keys` struct is not an enum,
// so a key added there for a type other than `R` is not covered.
fn mark(seen: &mut [bool; 4], summary: &SummaryModel) {
    let slot = match summary {
        SummaryModel::Count(_) => 0,
        SummaryModel::Distinct(_) => 1,
        SummaryModel::Sum(_) => 2,
        SummaryModel::Row(_) => 3,
    };
    seen[slot] = true;
}

static TOP: Field = Field::Object(&ROOT);
static ELEMENTS: Field = Field::Object(&ELEMENT);
static SUMMARIES: Field = Field::Object(&SUMMARY);

/// The keys found at each object of a schema, by the schema's address, with
/// the first key the schema lists to name it.
type Keys = BTreeMap<usize, (&'static str, BTreeSet<String>)>;

fn record(keys: &mut Keys, schema: &'static ObjectSchema, found: impl Iterator<Item = String>) {
    let name = schema.keys.first().map_or("", |(key, _)| *key);
    let at = std::ptr::from_ref(schema) as usize;
    keys.entry(at)
        .or_insert((name, BTreeSet::new()))
        .1
        .extend(found);
}

// The keys a value holds at each object of the registry it is walked beside.
fn walk_value(keys: &mut Keys, field: &'static Field, value: &Value) {
    match (field, value) {
        (Field::Object(schema), Value::Object(members)) => {
            record(keys, schema, members.keys().cloned());
            for (key, inner) in schema.keys {
                if let Some(found) = members.get(*key) {
                    walk_value(keys, inner, found);
                }
            }
        }
        (Field::Map(item), Value::Object(members)) => {
            members
                .values()
                .for_each(|found| walk_value(keys, item, found));
        }
        (Field::List { item, .. }, Value::Array(items)) => {
            items.iter().for_each(|found| walk_value(keys, item, found));
        }
        _ => {}
    }
}

// The keys of each object reachable from the field; an object already seen is
// not entered again, since an element holds elements.
fn walk_registry(keys: &mut Keys, field: &'static Field) {
    match field {
        Field::Object(schema) => {
            let at = std::ptr::from_ref(*schema) as usize;
            if keys.contains_key(&at) {
                return;
            }
            record(
                keys,
                schema,
                schema.keys.iter().map(|(key, _)| key.to_string()),
            );
            schema
                .keys
                .iter()
                .for_each(|(_, inner)| walk_registry(keys, inner));
        }
        Field::Map(item) | Field::List { item, .. } => walk_registry(keys, item),
        Field::Leaf { .. } => {}
    }
}

// Only the registry side is walked: `check_shape` already refuses a key the
// registry does not list, naming its path.
fn difference(model: &Keys, registry: &Keys) -> Vec<String> {
    let none = BTreeSet::new();
    let mut report = Vec::new();
    for (at, (name, in_registry)) in registry {
        let in_model = model.get(at).map_or(&none, |(_, keys)| keys);
        if !in_registry.is_subset(in_model) {
            report.push(format!(
                "the object holding \"{name}\": only in the registry {:?}",
                in_registry.difference(in_model).collect::<Vec<_>>(),
            ));
        }
    }
    report
}

#[test]
fn the_model_and_the_registry_hold_the_same_keys() {
    let model = every_key_model();
    let mut seen = [false; 4];
    model
        .summaries
        .values()
        .for_each(|summary| mark(&mut seen, summary));
    assert!(
        seen.iter().all(|set| *set),
        "every_key_model misses a summary variant"
    );
    let value = to_value(model).unwrap();
    check_shape(&value).unwrap_or_else(|e| panic!("only in the model: {e}"));
    let (mut in_model, mut in_registry) = (Keys::new(), Keys::new());
    walk_value(&mut in_model, &TOP, &value);
    walk_registry(&mut in_registry, &ELEMENTS);
    walk_registry(&mut in_registry, &SUMMARIES);
    let report = difference(&in_model, &in_registry);
    assert!(report.is_empty(), "{}", report.join("\n"));
}
