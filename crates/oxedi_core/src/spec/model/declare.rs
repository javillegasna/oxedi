//! `spec_object!`: one declaration per spec object, which emits its public
//! model and its schema table, so the two cannot name different keys.

/// Declares one object of a spec: the header names the model, the schema
/// static and the `expecting` text of the model.
/// Each row is one key: its field, the key when it differs (`as "where"`),
/// the model type, the schema `Field`, and a presence word.
/// Attributes before a row, its docs and a model-side `#[serde(...)]`, go on the
/// model field.
///
/// ```text
/// spec_object! {
///     /// The object's docs.
///     ExampleModel, EXAMPLE, "an example object";
///     /// Docs of the key.
///     segment: String, TEXT, required;
///     max: Option<usize>, nullable(Leaf::Count), optional;
///     codes: BTreeMap<String, Vec<String>>, Field::Map(&TEXTS), default;
///     usage: Usage, nullable(Leaf::Text), default_or_null;
/// }
/// ```
///
/// `required` puts the key in the schema's `required` list. `optional` (an
/// `Option`) leaves `None` unwritten. `default` reads an absent key as its
/// default and leaves the default unwritten. `default_or_null` also reads
/// `null` as the default. Any other word fails to compile, naming the key, and
/// so does a row not in that form.
///
/// The rows are munched one at a time: `@rows` reads a row into a fixed form,
/// `@word` turns its presence word into serde attributes, `@field` appends the
/// field to the model and the key to the schema, and the last `@rows` emits
/// the items.
macro_rules! spec_object {
    (
        $(#[$doc:meta])*
        $model:ident, $schema:ident, $expecting:literal;
        $($rows:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @rows [$(#[$doc])* $model, $schema, $expecting] {} [] [] $($rows)*
        }
    };
    (
        @rows $head:tt $mf:tt $keys:tt $req:tt
        $(#[$fdoc:meta])* $field:ident $(as $key:literal)? : $mty:ty, $kind:expr, $word:ident;
        $($rest:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @word $word $head $mf $keys $req [$(#[$fdoc])*] $field [$($key)?]
            ($mty) ($kind) $($rest)*
        }
    };
    (
        @rows $head:tt $mf:tt $keys:tt $req:tt
        $(#[$fdoc:meta])* $field:ident $($row:tt)+
    ) => {
        compile_error!(concat!(
            "spec_object!: the row of `", stringify!($field), "` is not `field $(as \"key\")? : ",
            "Model, Field, presence;`"
        ));
    };
    (
        @word required $head:tt $mf:tt $keys:tt [$($req:tt)*]
        $fdoc:tt $field:ident [$($key:literal)?] $($row:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @field [] $head $mf $keys
            [$($req)* $crate::spec::model::declare::spec_object!(@key $field $($key)?),]
            $fdoc $field [$($key)?] $($row)*
        }
    };
    (@word optional $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(skip_serializing_if = "Option::is_none")]] $($row)*
        }
    };
    (@word default $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(default, skip_serializing_if = "crate::spec::model::is_default")]]
            $($row)*
        }
    };
    (@word default_or_null $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(
                default,
                deserialize_with = "crate::spec::model::null_is_default",
                skip_serializing_if = "crate::spec::model::is_default"
            )]] $($row)*
        }
    };
    (
        @word $other:ident $head:tt $mf:tt $keys:tt $req:tt
        $fdoc:tt $field:ident $($row:tt)*
    ) => {
        compile_error!(concat!(
            "spec_object!: the row of `", stringify!($field), "` ends in `", stringify!($other),
            "`; a row ends in one of required, optional, default, default_or_null"
        ));
    };
    (
        @field [$($mattr:tt)*] $head:tt {$($mf:tt)*} [$($keys:tt)*]
        $req:tt [$($fdoc:tt)*] $field:ident [$($key:literal)?]
        ($mty:ty) ($kind:expr) $($rest:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @rows $head
            {$($mf)* $($fdoc)* $($mattr)* $(#[serde(rename = $key)])? pub $field: $mty,}
            [$($keys)* ($crate::spec::model::declare::spec_object!(@key $field $($key)?), $kind),]
            $req $($rest)*
        }
    };
    (@key $field:ident) => {
        stringify!($field)
    };
    (@key $field:ident $key:literal) => {
        $key
    };
    (
        @rows [$(#[$doc:meta])* $model:ident, $schema:ident, $expecting:literal]
        {$($mf:tt)*} [$($keys:tt)*] [$($req:tt)*]
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(deny_unknown_fields, expecting = $expecting)]
        pub struct $model {
            $($mf)*
        }

        #[allow(
            dead_code,
            reason = "the shape walk reaches only the schemas of the sections it still checks"
        )]
        pub(in crate::spec) static $schema: $crate::spec::shape::ObjectSchema =
            $crate::spec::shape::ObjectSchema {
                keys: &[$($keys)*],
                required: &[$($req)*],
            };
    };
}

pub(super) use spec_object;
