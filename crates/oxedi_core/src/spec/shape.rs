//! The schema tables as data: every object of a spec is an [`ObjectSchema`], a table of its
//! keys and the [`Field`] each holds, emitted with its model in `model` (the helpers that build
//! a `Field` are in `sections`). The model read checks a spec and nothing reads
//! these tables.
#![allow(
    dead_code,
    reason = "the schema tables are emitted with each model and nothing reads them"
)]

/// The scalar kinds the model expects.
#[derive(Clone, Copy)]
pub(super) enum Leaf {
    Text,
    Flag,
    Count,
    Pick,
}

/// What a key of an object holds.
#[derive(Clone, Copy)]
pub(super) enum Field {
    /// A scalar of the kind; `null` passes too when `nullable`.
    Leaf { kind: Leaf, nullable: bool },
    /// An array whose every item is the field; `expected` describes the
    /// array in the error when the value is not one.
    List {
        item: &'static Field,
        expected: &'static str,
    },
    /// An object whose every member, whatever its key, is the field.
    Map(&'static Field),
    /// An object of the schema.
    Object(&'static ObjectSchema),
    /// A value only the model read checks.
    Opaque,
}

/// The keys an object may hold, in the order their values are checked, and
/// the keys it must hold.
pub(super) struct ObjectSchema {
    pub(super) keys: &'static [(&'static str, Field)],
    pub(super) required: &'static [&'static str],
}
