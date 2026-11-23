//! Unit tests of the package module, one file per topic.

mod header;
mod id;
mod layers;
mod owners;
mod stack;

use super::Package;

/// A spec that reads as a whole model: one loop, one segment.
const ENVELOPE_SPEC: &str = r#"{"loops":{"a":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0}}}},"segments":{"AA":{"elements":{"1":{"name":"code","type":"ID","codes":["X","Y"]}}}}}"#;

/// A package with the given id, layers, bases and spec text.
fn package(id: &str, layers: &[&str], bases: &[&str], spec: &str) -> Package {
    let words = |items: &[&str]| format!("{items:?}");
    let text = format!(
        r#"{{"package":{{"format":1,"id":"{id}","layers":{},"bases":{}}},"spec":{spec}}}"#,
        words(layers),
        words(bases)
    );
    Package::from_json(&text).unwrap()
}

/// The envelope every stack of the tests starts from.
fn envelope() -> Package {
    package("test/envelope/005010", &["envelope"], &[], ENVELOPE_SPEC)
}
