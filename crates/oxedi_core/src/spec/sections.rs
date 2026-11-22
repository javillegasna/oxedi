//! The helpers that build a schema `Field`. Every object's schema is emitted
//! with its model in `model`; `shape` holds the tables.

use super::shape::{Field, Leaf};

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
