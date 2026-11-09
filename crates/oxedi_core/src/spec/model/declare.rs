//! `spec_object!`: one declaration per spec object, which emits its public
//! model, the struct the loader deserializes and its registry schema, so the
//! three cannot name different keys.

/// Declares one object of a spec: the header names the model, the raw struct
/// (`Model / Raw` emits both; `Model = Raw` makes the raw name an alias of the
/// model), the registry static and the `expecting` text both structs share.
/// Each row is one key: its field, the key when it differs (`as "where"`),
/// the model type, the raw type and its attributes when they differ
/// (`| #[serde(...)] RawType`), the registry `Field`, and a presence word.
///
/// ```text
/// spec_object! {
///     /// The object's docs.
///     ExampleModel / RawExample, EXAMPLE, "an example object";
///     /// Docs of the key.
///     segment: String, TEXT, required;
///     max: Option<usize>, nullable(Leaf::Count), optional;
///     codes: BTreeMap<String, Vec<String>>, Field::Map(&TEXTS), default;
///     usage: Usage | Option<String>, nullable(Leaf::Text), default_or_null;
/// }
/// ```
///
/// `required` puts the key in the schema's `required` list. `optional` (an
/// `Option`) leaves `None` unwritten. `default` reads an absent key as its
/// default and leaves the default unwritten. `default_or_null` also reads
/// `null` as the default in the model; its raw type is an `Option`. Any other
/// word fails to compile, naming the key.
///
/// The rows are munched one at a time: `@rows` reads a row into a fixed form,
/// `@word` turns its presence word into serde attributes, `@field` appends the
/// field to both structs and the key to the schema, and the last `@rows`
/// emits the items.
macro_rules! spec_object {
    (
        $(#[$doc:meta])*
        $model:ident $sep:tt $raw:ident, $schema:ident, $expecting:literal;
        $($rows:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @rows [$(#[$doc])* $model $sep $raw, $schema, $expecting] {} {} [] [] $($rows)*
        }
    };
    (
        @rows $head:tt $mf:tt $rf:tt $keys:tt $req:tt
        $(#[$fdoc:meta])* $field:ident $(as $key:literal)?
            : $mty:ty | $(#[$rattr:meta])* $rty:ty, $kind:expr, $word:ident;
        $($rest:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @word $word $head $mf $rf $keys $req [$(#[$fdoc])*] $field [$($key)?]
            ($mty) [$(#[$rattr])*] ($rty) ($kind) $($rest)*
        }
    };
    (
        @rows $head:tt $mf:tt $rf:tt $keys:tt $req:tt
        $(#[$fdoc:meta])* $field:ident $(as $key:literal)? : $mty:ty, $kind:expr, $word:ident;
        $($rest:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @word $word $head $mf $rf $keys $req [$(#[$fdoc])*] $field [$($key)?]
            ($mty) [] ($mty) ($kind) $($rest)*
        }
    };
    (
        @word required $head:tt $mf:tt $rf:tt $keys:tt [$($req:tt)*]
        $fdoc:tt $field:ident [$($key:literal)?] $($row:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @field [] [] $head $mf $rf $keys
            [$($req)* $crate::spec::model::declare::spec_object!(@key $field $($key)?),]
            $fdoc $field [$($key)?] $($row)*
        }
    };
    (@word optional $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(skip_serializing_if = "Option::is_none")]] [] $($row)*
        }
    };
    (@word default $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(default, skip_serializing_if = "crate::spec::model::is_default")]]
            [#[serde(default)]] $($row)*
        }
    };
    (@word default_or_null $($row:tt)*) => {
        $crate::spec::model::declare::spec_object! {
            @field [#[serde(
                default,
                deserialize_with = "crate::spec::model::null_is_default",
                skip_serializing_if = "crate::spec::model::is_default"
            )]] [] $($row)*
        }
    };
    (
        @word $other:ident $head:tt $mf:tt $rf:tt $keys:tt $req:tt
        $fdoc:tt $field:ident $($row:tt)*
    ) => {
        compile_error!(concat!(
            "spec_object!: the row of `", stringify!($field), "` ends in `", stringify!($other),
            "`; a row ends in one of required, optional, default, default_or_null"
        ));
    };
    (
        @field [$($mattr:tt)*] [$($rword:tt)*] $head:tt {$($mf:tt)*} {$($rf:tt)*} [$($keys:tt)*]
        $req:tt [$($fdoc:tt)*] $field:ident [$($key:literal)?]
        ($mty:ty) [$($rattr:tt)*] ($rty:ty) ($kind:expr) $($rest:tt)*
    ) => {
        $crate::spec::model::declare::spec_object! {
            @rows $head
            {$($mf)* $($fdoc)* $($mattr)* $(#[serde(rename = $key)])? pub $field: $mty,}
            {
                $($rf)* $($rword)* $($rattr)* $(#[serde(rename = $key)])?
                pub(in crate::spec) $field: $rty,
            }
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
        @rows [$(#[$doc:meta])* $model:ident / $raw:ident, $schema:ident, $expecting:literal]
        $mf:tt {$($rf:tt)*} $keys:tt $req:tt
    ) => {
        $crate::spec::model::declare::spec_object! {
            @emit [$(#[$doc])* $model, $schema, $expecting] $mf $keys $req
        }

        #[derive(Debug, ::serde::Deserialize)]
        #[serde(deny_unknown_fields, expecting = $expecting)]
        pub(in crate::spec) struct $raw {
            $($rf)*
        }
    };
    (
        @rows [$(#[$doc:meta])* $model:ident = $raw:ident, $schema:ident, $expecting:literal]
        $mf:tt $rf:tt $keys:tt $req:tt
    ) => {
        $crate::spec::model::declare::spec_object! {
            @emit [$(#[$doc])* $model, $schema, $expecting] $mf $keys $req
        }

        pub(in crate::spec) type $raw = $model;
    };
    (
        @emit [$(#[$doc:meta])* $model:ident, $schema:ident, $expecting:literal]
        {$($mf:tt)*} [$($keys:tt)*] [$($req:tt)*]
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(deny_unknown_fields, expecting = $expecting)]
        pub struct $model {
            $($mf)*
        }

        pub(in crate::spec) static $schema: $crate::spec::shape::ObjectSchema =
            $crate::spec::shape::ObjectSchema {
                keys: &[$($keys)*],
                required: &[$($req)*],
            };
    };
}

pub(super) use spec_object;
