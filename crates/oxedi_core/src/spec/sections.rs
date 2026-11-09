//! The JSON schema of a spec as data: one [`ObjectSchema`] per object the
//! spec holds, each listing its keys in the order their values are checked.
//! [`ROOT`] lists the sections of a spec; `shape` walks a value against it.

use super::model::{
    BALANCE, LOOP, SEGMENT, SUMMARY_COLUMN, SUMMARY_ROW, SUMMARY_TABLE, TABLE, VERSION,
};
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

/// The sections of a spec.
pub(super) static ROOT: ObjectSchema = ObjectSchema {
    keys: &[
        ("name", TEXT),
        ("version", Field::Object(&VERSION)),
        ("loops", Field::Map(&Field::Object(&LOOP))),
        ("tables", Field::Map(&Field::Object(&TABLE))),
        ("balancing", Field::Map(&Field::Object(&BALANCE))),
        ("segments", Field::Map(&Field::Object(&SEGMENT))),
        ("summaries", Field::Map(&Field::Object(&SUMMARY))),
        ("default_table", TEXT),
    ],
    required: &["name", "loops"],
};

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

static SUMMARY: ObjectSchema = ObjectSchema {
    keys: &[
        ("count", Field::Object(&SUMMARY_TABLE)),
        ("distinct", Field::Object(&SUMMARY_COLUMN)),
        ("sum", Field::Object(&SUMMARY_COLUMN)),
        ("row", Field::Object(&SUMMARY_ROW)),
    ],
    required: &[],
};
