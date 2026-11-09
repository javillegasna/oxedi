//! The spec model and the section registry name the same keys at every
//! object, so a key added to one and not the other fails here.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, to_value};

use crate::mask::Phi;
use crate::spec::sections::ROOT;
use crate::spec::shape::{Field, ObjectSchema, check_shape};
use crate::spec::{
    BalanceModel, ColumnModel, ControlCount, ControlModel, ElementModel, ElementPosition,
    LoopModel, OccurrenceModel, Pick, QualifierModel, RepeatModel, SegmentModel, Sign, SpecModel,
    SummaryColumnModel, SummaryFieldModel, SummaryModel, SummaryRowModel, SummaryTableModel,
    TableModel, TermModel, TriggerModel, Usage, VersionModel,
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

fn occurrence() -> OccurrenceModel {
    OccurrenceModel {
        segment: text("CLP"),
        pos: 0,
        usage: Usage::Required,
        max: Some(2),
        qualifier: Some(QualifierModel {
            element: 1,
            component: Some(2),
            codes: vec![text("Q")],
        }),
        codes: one(
            ElementPosition {
                element: 1,
                component: Some(2),
            },
            vec![text("C")],
        ),
    }
}

fn loop_model() -> LoopModel {
    LoopModel {
        parent: Some(text("2000")),
        trigger: TriggerModel {
            segment: text("CLP"),
            conditions: one(1, text("X")),
        },
        occurrences: one(text("clp"), occurrence()),
        max: Some(3),
        usage: Usage::Required,
        end: Some(text("SE")),
        control: Some(ControlModel {
            opener_element: 1,
            closer_element: 2,
            count_element: 3,
            count: ControlCount::Segments,
        }),
    }
}

fn column() -> ColumnModel {
    ColumnModel {
        loop_name: Some(text("2100")),
        segment: Some(text("CLP")),
        conditions: one(1, text("X")),
        occurrence: Some(text("clp")),
        pick: Some(Pick::Nth(2)),
        element: Some(1),
        component: Some(2),
        group_element: Some(0),
        segment_index: true,
        phi: Some(Phi::Name),
    }
}

fn term() -> TermModel {
    TermModel {
        loop_name: Some(text("2100")),
        occurrence: text("clp"),
        elements: vec![1],
        component: Some(2),
        sign: Sign::Minus,
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
        version: Some(VersionModel {
            segment: text("GS"),
            element: 8,
            values: vec![text("005010")],
        }),
        loops: one(text("2100"), loop_model()),
        segments: one(
            text("CLP"),
            SegmentModel {
                elements: one(1, element(one(1, element(BTreeMap::new())))),
            },
        ),
        tables: one(
            text("claims"),
            TableModel {
                loops: vec![text("2100")],
                reference: Some(text("claim")),
                segment: Some(text("CLP")),
                repeat: Some(RepeatModel { from: 2, step: 2 }),
                columns: one(text("c"), column()),
                phi_seed: Some(text("c")),
            },
        ),
        balancing: one(
            text("balance"),
            BalanceModel {
                per: text("2100"),
                target: vec![term()],
                sum: vec![term()],
            },
        ),
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
        default_table: Some(text("claims")),
    }
}

static TOP: Field = Field::Object(&ROOT);

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
    let value = to_value(every_key_model()).unwrap();
    check_shape(&value).unwrap();
    let (mut in_model, mut in_registry) = (Keys::new(), Keys::new());
    walk_value(&mut in_model, &TOP, &value);
    walk_registry(&mut in_registry, &TOP);
    let report = difference(&in_model, &in_registry);
    assert!(report.is_empty(), "{}", report.join("\n"));
}
