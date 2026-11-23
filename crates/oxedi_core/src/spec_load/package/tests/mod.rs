//! Unit tests of the package module, one file per topic.

mod change;
mod checked;
mod header;
mod id;
mod layers;
mod narrow;
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

/// An envelope's spec: a loop with two occurrences, a segment `AA` whose first
/// element bounds everything and whose second is a composite with one
/// component, a version and a balancing rule.
const BASE: &str = r#"{"version":{"segment":"AA","element":1,"values":["V"]},"loops":{"a":{"trigger":{"segment":"AA"},"usage":"required","max":2,"occurrences":{"aa":{"segment":"AA","pos":0,"usage":"required","max":1,"codes":{"1":["X"]}},"ab":{"segment":"AA","pos":1,"qualifier":{"element":1,"codes":["Y"]}}}}},"segments":{"AA":{"elements":{"1":{"name":"code","type":"ID","min":1,"max":2,"required":true,"codes":["X","Y"]},"2":{"name":"pair","type":"AN","composite":{"1":{"name":"part","type":"ID","codes":["P","Q"]}}}}}},"balancing":{"r":{"per":"a","target":[{"occurrence":"aa","elements":[1]}],"sum":[{"occurrence":"ab","elements":[1]}]}}}"#;
