//! `spec_object!`: one declaration per spec object, which emits its public
//! model with the serde attributes of each key's presence.

/// Declares one object of a spec: the header names the model and its
/// `expecting` text.
/// Each row is one key: its field, the key when it differs (`as "where"`),
/// the model type and a presence word.
/// Attributes before a row, its docs and a model-side `#[serde(...)]`, go on the
/// model field.
///
/// ```text
/// spec_object! {
///     /// The object's docs.
///     ExampleModel, "an example object";
///     /// Docs of the key.
///     segment: String, required;
///     max: Option<usize>, optional;
///     codes: BTreeMap<String, Vec<String>>, default;
///     usage: Usage, default_or_null;
/// }
/// ```
///
/// `required` reads an absent key as missing. `optional` (an `Option`) leaves
/// `None` unwritten. `default` reads an absent key as its default and leaves
/// the default unwritten. `default_or_null` also reads `null` as the default.
/// Any other word fails to compile, naming the key, and so does a row not in
/// that form.
///
/// The rows are munched one at a time: `@rows` reads a row into a fixed form,
/// `@word` turns its presence word into serde attributes, `@field` appends the
/// field to the model, and the last `@rows` emits the model.
macro_rules! spec_object {
    (
        $(#[$doc:meta])*
        $model:ident, $expecting:literal;
        $($rows:tt)*
    ) => {
        $crate::spec_load::model::declare::spec_object! {
            @rows [$(#[$doc])* $model, $expecting] {} $($rows)*
        }
    };
    (
        @rows $head:tt $mf:tt
        $(#[$fdoc:meta])* $field:ident $(as $key:literal)? : $mty:ty, $word:ident;
        $($rest:tt)*
    ) => {
        $crate::spec_load::model::declare::spec_object! {
            @word $word $head $mf [$(#[$fdoc])*] $field [$($key)?] ($mty) $($rest)*
        }
    };
    (
        @rows $head:tt $mf:tt
        $(#[$fdoc:meta])* $field:ident $($row:tt)+
    ) => {
        compile_error!(concat!(
            "spec_object!: the row of `", stringify!($field), "` is not `field $(as \"key\")? : ",
            "Model, presence;`"
        ));
    };
    (@word required $($row:tt)*) => {
        $crate::spec_load::model::declare::spec_object! { @field [] $($row)* }
    };
    (@word optional $($row:tt)*) => {
        $crate::spec_load::model::declare::spec_object! {
            @field [#[serde(skip_serializing_if = "Option::is_none")]] $($row)*
        }
    };
    (@word default $($row:tt)*) => {
        $crate::spec_load::model::declare::spec_object! {
            @field [#[serde(default, skip_serializing_if = "crate::spec_load::model::is_default")]]
            $($row)*
        }
    };
    (@word default_or_null $($row:tt)*) => {
        $crate::spec_load::model::declare::spec_object! {
            @field [#[serde(
                default,
                deserialize_with = "crate::spec_load::model::null_is_default",
                skip_serializing_if = "crate::spec_load::model::is_default"
            )]] $($row)*
        }
    };
    (@word $other:ident $head:tt $mf:tt $fdoc:tt $field:ident $($row:tt)*) => {
        compile_error!(concat!(
            "spec_object!: the row of `", stringify!($field), "` ends in `", stringify!($other),
            "`; a row ends in one of required, optional, default, default_or_null"
        ));
    };
    (
        @field [$($mattr:tt)*] $head:tt {$($mf:tt)*}
        [$($fdoc:tt)*] $field:ident [$($key:literal)?] ($mty:ty) $($rest:tt)*
    ) => {
        $crate::spec_load::model::declare::spec_object! {
            @rows $head
            {$($mf)* $($fdoc)* $($mattr)* $(#[serde(rename = $key)])? pub $field: $mty,}
            $($rest)*
        }
    };
    (@rows [$(#[$doc:meta])* $model:ident, $expecting:literal] {$($mf:tt)*}) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(deny_unknown_fields, expecting = $expecting)]
        pub struct $model {
            $($mf)*
        }
    };
}

pub(super) use spec_object;
