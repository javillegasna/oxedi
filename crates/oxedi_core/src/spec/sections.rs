//! The helpers that build a registry `Field`, and the two objects no
//! declaration emits: `ELEMENT` and `SUMMARY`. Every other object's schema is
//! emitted with its model in `model`; `shape` walks a value against them.

use super::model::{SUMMARY_COLUMN, SUMMARY_ROW, SUMMARY_TABLE};
use super::shape::{Field, Leaf, ObjectSchema};

pub(super) const fn leaf(kind: Leaf) -> Field {
    Field::Leaf {
        kind,
        nullable: false,
    }
}

pub(super) const fn nullable(kind: Leaf) -> Field {
    Field::Leaf {
        kind,
        nullable: true,
    }
}

pub(super) const TEXT: Field = leaf(Leaf::Text);
pub(super) const COUNT: Field = leaf(Leaf::Count);

pub(super) const TEXTS: Field = Field::List {
    item: &TEXT,
    expected: "an array of strings",
};

pub(super) const COUNTS: Field = Field::List {
    item: &COUNT,
    expected: "an array of non-negative integers",
};

/// An array of objects of one schema.
macro_rules! objects {
    ($schema:expr) => {
        Field::List {
            item: &Field::Object($schema),
            expected: "an array of objects",
        }
    };
}

pub(super) use objects;

pub(super) static ELEMENT: ObjectSchema = ObjectSchema {
    keys: &[
        ("codes", TEXTS),
        ("name", TEXT),
        ("type", TEXT),
        ("required", leaf(Leaf::Flag)),
        ("min", nullable(Leaf::Count)),
        ("max", nullable(Leaf::Count)),
        ("scale", nullable(Leaf::Byte)),
        ("composite", Field::Map(&Field::Object(&ELEMENT))),
    ],
    required: &["name", "type"],
};

pub(super) static SUMMARY: ObjectSchema = ObjectSchema {
    keys: &[
        ("count", Field::Object(&SUMMARY_TABLE)),
        ("distinct", Field::Object(&SUMMARY_COLUMN)),
        ("sum", Field::Object(&SUMMARY_COLUMN)),
        ("row", Field::Object(&SUMMARY_ROW)),
    ],
    required: &[],
};
