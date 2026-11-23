//! Small specs shared by the unit tests: a four-table spec for the spec and the
//! projector, a three-loop nested spec for the engine and the tree, a one-loop
//! spec that declares a version, and the tokenizer, loaders and patch helper
//! the tests share.

use std::sync::Arc;

use super::Spec;
use crate::segment::Segment;
use crate::spec_load::{Builtin, LoadedSpec, SpecError, Version};
use crate::{Delimiters, Tokenizer};

pub(crate) const SPEC: &str = r#"{"name":"t",
    "loops":{
        "head":{"trigger":{"segment":"HD"},"occurrences":{"hd":{"segment":"HD","pos":0},"zz":{"segment":"ZZ","pos":1}},"end":"TR"},
        "note":{"parent":"head","trigger":{"segment":"NM","where":{"1":"P"}}},
        "claim":{"parent":"head","trigger":{"segment":"CL"},"occurrences":{"cl":{"segment":"CL","pos":0},"dt":{"segment":"DT","pos":1},"aj":{"segment":"AJ","pos":2},"rf":{"segment":"RF","pos":3}}},
        "line":{"parent":"claim","trigger":{"segment":"LN"},"occurrences":{"ln":{"segment":"LN","pos":0},"dt":{"segment":"DT","pos":1},"aj":{"segment":"AJ","pos":2}}}
    },
    "segments":{
        "HD":{"elements":{"1":{"name":"batch","type":"AN","usage":"required","min":1,"max":5}}},
        "CL":{"elements":{
            "1":{"name":"claim_id","type":"AN","usage":"required","min":1,"max":10},
            "2":{"name":"charge","type":"R","usage":"required","max":10},
            "3":{"name":"units","type":"N0","max":2},
            "4":{"name":"procedure","type":"AN","composite":{
                "1":{"name":"qualifier","type":"ID","usage":"required","min":2,"max":2},
                "2":{"name":"code","type":"AN","usage":"required","max":5}
            }}
        }},
        "DT":{"elements":{
            "1":{"name":"qualifier","type":"ID","usage":"required"},
            "2":{"name":"date","type":"DT"},
            "3":{"name":"time","type":"TM"}
        }},
        "AJ":{"elements":{
            "1":{"name":"group","type":"ID","usage":"required"},
            "2":{"name":"reason","type":"ID"},
            "3":{"name":"amount","type":"R"},
            "4":{"name":"reason_2","type":"ID"},
            "5":{"name":"amount_2","type":"R"}
        }}
    },
    "tables":{
        "heads":{"loops":["head"],"ref":"head","columns":{
            "batch":{"segment":"HD","element":1},
            "payer":{"loop":"note","segment":"NM","element":2}
        }},
        "claims":{"loops":["claim"],"ref":"claim","columns":{
            "claim_id":{"segment":"CL","element":1},
            "charge":{"segment":"CL","element":2},
            "units":{"segment":"CL","element":3},
            "procedure":{"segment":"CL","element":4},
            "code":{"segment":"CL","element":4,"component":2},
            "from":{"segment":"DT","where":{"1":"150"},"element":2},
            "reference":{"segment":"RF","element":2},
            "first_line_at":{"loop":"line","segment":"LN","segment_index":true}
        }},
        "lines":{"loops":["line"],"ref":"line","columns":{
            "code":{"segment":"LN","element":1},
            "date":{"segment":"DT","element":2},
            "time":{"segment":"DT","element":3}
        }},
        "adjustments":{"loops":["claim","line"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
            "group":{"element":1},
            "reason":{"group_element":0},
            "amount":{"group_element":1}
        }}
    }
}"#;

pub(crate) fn spec() -> Spec {
    Spec::from_json(SPEC).unwrap()
}

pub(crate) const NESTED: &str = r#"{"name":"t","loops":{
    "A":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0},"a1":{"segment":"A1","pos":1}},"end":"AE"},
    "B":{"parent":"A","trigger":{"segment":"BB"},"occurrences":{"bb":{"segment":"BB","pos":0},"b1":{"segment":"B1","pos":1}}},
    "C":{"parent":"B","trigger":{"segment":"CC","where":{"1":"X"}},"occurrences":{"cc":{"segment":"CC","pos":0},"c1":{"segment":"C1","pos":1}}}
}}"#;

pub(crate) fn nested() -> Spec {
    Spec::from_json(NESTED).unwrap()
}

/// The built-in 835 at `version`, its model beside its spec.
pub(crate) fn builtin(version: Version) -> LoadedSpec {
    Builtin::find("835", version)
        .expect("the registry ships the 835 at every version")
        .loaded()
        .expect("a built-in's model reads")
}

/// `json` read into its model and compiled.
pub(crate) fn loaded(json: &str) -> LoadedSpec {
    LoadedSpec::from_json(json).expect("the spec loads")
}

/// `base` with the merge patch `patch` over its model, compiled; fails as
/// `LoadedSpec::merge_patch` does.
pub(crate) fn patched(base: &LoadedSpec, patch: &str) -> Result<Spec, SpecError> {
    base.merge_patch(patch)
        .map(|loaded| Arc::unwrap_or_clone(loaded.spec))
}

/// The segments of `input`, delimited by `*`, `:` and `~`.
pub(crate) fn segs(input: &[u8]) -> Vec<Segment<'_>> {
    Tokenizer::with_delimiters(input, Delimiters::new(b'*', b':', b'~')).collect()
}

/// The JSON of a one-loop spec named `name` whose `version` is the JSON
/// `version`, or that declares none when `version` is empty.
pub(crate) fn declared(name: &str, version: &str) -> String {
    let version = if version.is_empty() {
        String::new()
    } else {
        format!(r#","version":{version}"#)
    };
    format!(r#"{{"name":"{name}","loops":{{"a":{{"trigger":{{"segment":"GS"}}}}}}{version}}}"#)
}

/// [`declared`], compiled.
pub(crate) fn declaring(name: &str, version: &str) -> Spec {
    Spec::from_json(&declared(name, version)).unwrap()
}
