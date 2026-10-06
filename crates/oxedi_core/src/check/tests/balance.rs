//! Balancing checks over a small spec: a `pay` loop whose `PY02` must equal
//! the claim payments `CL04` minus the adjustments `AJ02`/`AJ04`, and a
//! `claim` loop whose `CL03 - CL04` must equal its own and its lines'
//! adjustments `CA02`/`CA04`.

use super::check_all;
use crate::check::*;

const SPEC: &str = r#"{"name":"b",
    "loops":{
        "pay":{"trigger":{"segment":"PY"},"end":"PE","occurrences":{
            "py":{"segment":"PY","pos":1,"usage":"required","max":1},
            "aj":{"segment":"AJ","pos":90}
        }},
        "claim":{"parent":"pay","trigger":{"segment":"CL"},"occurrences":{
            "cl":{"segment":"CL","pos":10,"usage":"required","max":1},
            "ca":{"segment":"CA","pos":11}
        }},
        "line":{"parent":"claim","trigger":{"segment":"LN"},"occurrences":{
            "ln":{"segment":"LN","pos":20,"usage":"required","max":1},
            "ca":{"segment":"CA","pos":21}
        }}
    },
    "segments":{
        "PY":{"elements":{"2":{"name":"total","type":"R","required":true}}},
        "CL":{"elements":{
            "3":{"name":"charge","type":"R","required":true},
            "4":{"name":"paid","type":"R","required":true}
        }},
        "CA":{"elements":{
            "2":{"name":"amount","type":"R","required":true},
            "4":{"name":"amount_2","type":"R"}
        }},
        "AJ":{"elements":{
            "2":{"name":"amount","type":"R","required":true},
            "4":{"name":"amount_2","type":"R","scale":3}
        }}
    },
    "balancing":{
        "claim_rule":{"per":"claim",
            "target":[{"occurrence":"cl","elements":[3]},{"occurrence":"cl","elements":[4],"sign":"-"}],
            "sum":[{"occurrence":"ca","elements":[2,4]},{"loop":"line","occurrence":"ca","elements":[2,4]}]},
        "pay_rule":{"per":"pay",
            "target":[{"occurrence":"py","elements":[2]}],
            "sum":[{"loop":"claim","occurrence":"cl","elements":[4]},{"occurrence":"aj","elements":[2,4],"sign":"-"}]}
    }
}"#;

fn spec() -> Spec {
    Spec::from_json(SPEC).unwrap()
}

fn rendered(input: &str) -> Vec<String> {
    check_all(&spec(), input)
        .iter()
        .filter(|diagnostic| diagnostic.level == crate::SnipLevel::L3)
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_rules_compile_with_the_largest_scale_of_their_elements() {
    let spec = spec();
    let rules = spec.balancing();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].name, "claim_rule");
    assert_eq!(rules[0].scale, 2);
    assert_eq!(rules[1].name, "pay_rule");
    assert_eq!(rules[1].scale, 3);
    assert_eq!(rules[1].sum[1].elements, vec![2, 4]);
    assert!(rules[1].sum[1].negative);
}

#[test]
fn balanced_instances_yield_nothing() {
    // claim 1: 100 - 60 = 25 + 15 (its own) ; claim 2: 50 - 50 = 0, no
    // adjustment; pay: 60 + 50 - (5 + 5.000) = 100.
    assert_eq!(
        rendered("PY*x*100~CL*a*x*100*60~CA*x*25~LN~CA*x*10*x*5~CL*b*x*50*50~AJ*x*5*x*5.000~PE~"),
        Vec::<String>::new()
    );
}

#[test]
fn an_absent_optional_amount_counts_as_zero() {
    assert_eq!(
        rendered("PY*x*40~CL*a*x*100*40~CA*x*60*x~PE~"),
        Vec::<String>::new()
    );
}

#[test]
fn a_claim_that_does_not_add_up_names_the_rule_the_amounts_and_the_segments() {
    assert_eq!(
        rendered("PY*x*40~CL*a*x*100*40~CA*x*25~LN~CA*x*10*x*5~PE~"),
        vec![
            "SNIP 3 · balancing rule \"claim_rule\" fails in loop \"claim\" opened at segment #1: CL03 of claim \"cl\" - CL04 of claim \"cl\" is 60.00, but sum of CA02, CA04 of claim \"ca\" + sum of CA02, CA04 of line \"ca\" adds up to 40.00 (off by 20.00); read from segments #1, #2, #4 · segment #1, element 3 · at pay#1/claim#1 · datum \"100\""
        ]
    );
}

#[test]
fn the_payment_rule_adds_every_claim_and_subtracts_the_adjustments() {
    assert_eq!(
        rendered("PY*x*100.5~CL*a*x*60*60~CL*b*x*50*50~AJ*x*-1~PE~"),
        vec![
            "SNIP 3 · balancing rule \"pay_rule\" fails in loop \"pay\" opened at segment #0: PY02 of pay \"py\" is 100.500, but sum of CL04 of claim \"cl\" - sum of AJ02, AJ04 of pay \"aj\" adds up to 111.000 (off by -10.500); read from segments #0, #1, #2, #3 · segment #0, element 2 · at pay#1 · datum \"100.5\""
        ]
    );
}

#[test]
fn a_long_segment_list_ends_with_the_count_of_the_rest() {
    let claims = "CL*a*x*1*1~".repeat(12);
    assert_eq!(
        rendered(&format!("PY*x*1~{claims}PE~")),
        vec![
            "SNIP 3 · balancing rule \"pay_rule\" fails in loop \"pay\" opened at segment #0: PY02 of pay \"py\" is 1.000, but sum of CL04 of claim \"cl\" - sum of AJ02, AJ04 of pay \"aj\" adds up to 12.000 (off by -11.000); read from segments #0, #1, #2, #3, #4, #5, #6, #7, #8, #9 and 3 more · segment #0, element 2 · at pay#1 · datum \"1\""
        ]
    );
}

#[test]
fn an_instance_that_cannot_be_exact_is_left_to_its_other_findings() {
    // A required amount empty, a value that is not a decimal, a value with
    // more places than the rule's scale, and a missing target occurrence.
    for (input, rule) in [
        ("PY*x*1~CL*a*x*100*~PE~", "claim_rule"),
        ("PY*x*1~CL*a*x*100*40~CA*x*ABC~PE~", "claim_rule"),
        ("PY*x*1~CL*a*x*100*40~CA*x*60.001~PE~", "claim_rule"),
        ("CL*a*x*100*40~CA*x*60~CL*b*x*2*1~", "pay_rule"),
    ] {
        let found: Vec<String> = rendered(input)
            .into_iter()
            .filter(|line| line.contains(rule))
            .collect();
        assert_eq!(found, Vec::<String>::new(), "{input}");
    }
}

#[test]
fn an_unclosed_instance_is_checked_at_the_end_of_the_stream() {
    assert_eq!(
        rendered("PY*x*2~CL*a*x*1*1~"),
        vec![
            "SNIP 3 · balancing rule \"pay_rule\" fails in loop \"pay\" opened at segment #0: PY02 of pay \"py\" is 2.000, but sum of CL04 of claim \"cl\" - sum of AJ02, AJ04 of pay \"aj\" adds up to 1.000 (off by 1.000); read from segments #0, #1 · segment #0, element 2 · at pay#1 · datum \"2\""
        ]
    );
}

#[test]
fn a_fresh_stream_after_finish_starts_from_zero() {
    let spec = spec();
    let input = "PY*x*1~CL*a*x*1*1~PE~";
    let first = check_all(&spec, input);
    assert_eq!(first, check_all(&spec, input));
    assert!(first.iter().all(|d| d.level != crate::SnipLevel::L3));
}
