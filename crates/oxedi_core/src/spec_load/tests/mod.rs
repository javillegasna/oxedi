//! Unit tests of loading a spec, one file per topic.

mod balancing;
mod builtin;
mod codes;
mod columns;
mod controls;
mod declaration;
mod display;
mod loading;
mod model;
mod occurrences;
mod patch;
mod phi;
mod round_trip;
mod segments;
mod shape;
mod summaries;
mod tables;
mod triggers;
mod version;
mod vocabulary;

use crate::spec::Spec;
use crate::spec_load::SpecError;
use crate::spec_load::builtin::PACKAGES;
use crate::spec_load::package::Stack;
use crate::spec_load::{LoadedSpec, Package};

/// Every package the crate ships, read with its header.
pub(in crate::spec_load) fn shipped() -> Vec<Package> {
    PACKAGES
        .iter()
        .map(|text| Package::from_json(text).unwrap())
        .collect()
}

/// The compat patch package, as shipped.
pub(in crate::spec_load) const COMPAT: &str = include_str!("../../../specs/edi_835_parser.json");

/// The compat patch composed over the shipped stack and compiled.
pub(in crate::spec_load) fn compat() -> LoadedSpec {
    let pool = shipped();
    let top = Package::from_json(COMPAT).unwrap();
    let composed = Stack::resolve(&top, &pool)
        .unwrap()
        .compose_checked()
        .unwrap();
    LoadedSpec::from_model(composed.model).unwrap()
}

/// The package of `pool` whose id is `id`.
pub(in crate::spec_load) fn package_of<'p>(pool: &'p [Package], id: &str) -> &'p Package {
    pool.iter()
        .find(|package| package.id().as_str() == id)
        .unwrap_or_else(|| panic!("no shipped package {id}"))
}

pub(super) const CLP_ONLY: &str = r#"{"name":"t",
        "loops":{"2100":{"trigger":{"segment":"CLP"},"occurrences":{"clp":{"segment":"CLP","pos":0},"zz1":{"segment":"ZZ1","pos":1}}}},
        "segments":{"CLP":{"elements":{
            "1":{"name":"claim_submitter_id","type":"AN","usage":"required","min":1,"max":38},
            "3":{"name":"total_claim_charge_amount","type":"R","usage":"required"},
            "12":{"name":"drg_weight","type":"R","scale":4}
        }},
        "SVC":{"elements":{
            "1":{"name":"procedure","type":"AN","usage":"required","composite":{
                "1":{"name":"qualifier","type":"ID","usage":"required","min":2,"max":2},
                "2":{"name":"code","type":"AN","usage":"required"}
            }},
            "5":{"name":"units","type":"N0"}
        }}}
    }"#;

/// Each input fails with a `SpecError::Format` whose full text is the expected one.
pub(super) fn assert_model_faults(cases: &[(&str, &str)], load: impl Fn(&str) -> SpecError) {
    for (input, expected) in cases {
        let err = load(input);
        assert!(matches!(&err, SpecError::Format { .. }), "{input}: {err:?}");
        assert_eq!(err.to_string(), *expected, "{input}");
    }
}

pub(super) fn element_error(elements: &str) -> SpecError {
    let json = format!(
        r#"{{"name":"t","loops":{{"a":{{"trigger":{{"segment":"AA"}}}}}},"segments":{{"AA":{{"elements":{elements}}}}}}}"#
    );
    Spec::from_json(&json).unwrap_err()
}

pub(super) const TABLED: &str = r#"{"name":"t",
        "loops":{
            "A":{"trigger":{"segment":"AA"},"occurrences":{"aa":{"segment":"AA","pos":0},"a1":{"segment":"A1","pos":1}},"end":"AE"},
            "B":{"parent":"A","trigger":{"segment":"BB"},"occurrences":{"bb":{"segment":"BB","pos":0},"b1":{"segment":"B1","pos":1},"aj":{"segment":"AJ","pos":2}}},
            "C":{"parent":"B","trigger":{"segment":"CC"},"occurrences":{"cc":{"segment":"CC","pos":0},"c1":{"segment":"C1","pos":1},"aj":{"segment":"AJ","pos":2}}},
            "D":{"parent":"A","trigger":{"segment":"DD"}}
        },
        "segments":{
            "BB":{"elements":{"1":{"name":"id","type":"AN"},"2":{"name":"amount","type":"R"}}},
            "CC":{"elements":{"1":{"name":"code","type":"AN","composite":{
                "1":{"name":"qualifier","type":"ID"},"2":{"name":"value","type":"AN"}}}}}
        },
        "tables":{
            "heads":{"loops":["A"],"ref":"head","columns":{
                "code":{"segment":"AA","element":1},
                "note":{"loop":"D","segment":"DD","where":{"1":"N"},"element":2}
            }},
            "bodies":{"loops":["B"],"ref":"body","columns":{
                "id":{"segment":"BB","element":1},
                "line_at":{"loop":"C","segment":"C1","segment_index":true}
            }},
            "lines":{"loops":["C"],"ref":"line","columns":{
                "code":{"segment":"CC","element":1,"component":2}
            }},
            "adjustments":{"loops":["B","C"],"segment":"AJ","repeat":{"from":2,"step":2},"columns":{
                "kind":{"element":1},
                "reason":{"group_element":0},
                "amount":{"group_element":1}
            }}
        }
    }"#;

pub(super) fn table_error(tables: &str) -> SpecError {
    let json = format!(
        r#"{{"name":"t","loops":{{
                "A":{{"trigger":{{"segment":"AA"}}}},
                "B":{{"parent":"A","trigger":{{"segment":"BB"}}}},
                "C":{{"parent":"B","trigger":{{"segment":"CC"}},"occurrences":{{"cc":{{"segment":"CC","pos":0}},"xx":{{"segment":"XX","pos":1}}}}}},
                "D":{{"parent":"A","trigger":{{"segment":"DD"}},"occurrences":{{"dd":{{"segment":"DD","pos":0}},"xx":{{"segment":"XX","pos":1}}}}}}
            }},"tables":{tables}}}"#
    );
    Spec::from_json(&json).unwrap_err()
}
