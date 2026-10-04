//! Elements no column reads are validated without keeping their value; their
//! diagnostics must not depend on whether a column reads them.

use super::tests::{SPEC, delimiters, project, rendered, spec};
use super::*;
use proptest::prelude::*;

/// The test spec without its tables, so no column reads any element.
fn spec_without_tables() -> Spec {
    without_tables(SPEC)
}

/// `json` compiled without its tables.
fn without_tables(json: &str) -> Spec {
    let mut json: serde_json::Value = serde_json::from_str(json).unwrap();
    json.as_object_mut().unwrap().remove("tables");
    Spec::from_json(&json.to_string()).unwrap()
}

/// One element's read flag, then its components' as `(position, read)`.
type Flags = (usize, bool, Vec<(usize, bool)>);

/// The read flags of `id`'s plan, by element position.
fn flags(projector: &Projector<'_>, id: &[u8]) -> Vec<Flags> {
    projector
        .plans
        .get(id)
        .unwrap()
        .elements
        .iter()
        .map(|plan| {
            let components = plan
                .components
                .iter()
                .map(|&(at, _, _, read)| (at, read))
                .collect();
            (plan.position, plan.read, components)
        })
        .collect()
}

#[test]
fn read_flags_follow_the_columns_of_every_table() {
    let spec = spec();
    let projector = Projector::new(&spec, &delimiters());
    assert_eq!(
        flags(&projector, b"CL"),
        vec![
            (1, true, vec![]),
            (2, true, vec![]),
            (3, true, vec![]),
            (4, true, vec![(1, false), (2, true)]),
        ]
    );
    assert_eq!(
        flags(&projector, b"DT"),
        vec![(1, false, vec![]), (2, true, vec![]), (3, true, vec![])]
    );
    // `AJ` repeats groups of two from element 2: offsets 0 and 1 read every
    // group's elements, and element 1 is read by its own column.
    assert_eq!(
        flags(&projector, b"AJ"),
        vec![
            (1, true, vec![]),
            (2, true, vec![]),
            (3, true, vec![]),
            (4, true, vec![]),
            (5, true, vec![]),
        ]
    );
    assert_eq!(flags(&projector, b"HD"), vec![(1, true, vec![])]);

    let bare = spec_without_tables();
    let projector = Projector::new(&bare, &delimiters());
    assert_eq!(
        flags(&projector, b"CL"),
        vec![
            (1, false, vec![]),
            (2, false, vec![]),
            (3, false, vec![]),
            (4, false, vec![(1, false), (2, false)]),
        ]
    );
}

#[test]
fn unread_elements_raise_the_same_diagnostics_as_read_ones() {
    let input = "HD*TOOLONGBATCH~\
        CL*C1*12.345.6*123*HCPC:TOOLONGCODE:X~\
        DT**20240230*2460~DT*150*2024022*1261~\
        AJ**R1*1.2.3*R2*-~\
        LN*1~DT*X*abcdefgh*12~AJ*CO*R*-5.5~\
        CL**-~TR~";
    let (_, read) = project(&spec(), input);
    let (_, unread) = project(&spec_without_tables(), input);
    assert!(read.len() >= 10, "{:#?}", rendered(&read));
    assert_eq!(rendered(&unread), rendered(&read));
    assert_eq!(unread, read);
}

proptest! {
    #[test]
    fn diagnostics_do_not_depend_on_what_columns_read(
        values in proptest::collection::vec(
            proptest::collection::vec(
                proptest::sample::select(b"019-.:AZ".to_vec()),
                0..10,
            ),
            12,
        ),
    ) {
        let text: Vec<String> = values
            .iter()
            .map(|value| String::from_utf8(value.clone()).unwrap())
            .collect();
        let input = format!(
            "HD*{}~CL*{}*{}*{}*{}~DT*{}*{}*{}~AJ*{}*{}*{}*{}~TR~",
            text[0], text[1], text[2], text[3], text[4], text[5], text[6], text[7],
            text[8], text[9], text[10], text[11],
        );
        let (_, read) = project(&spec(), &input);
        let (_, unread) = project(&spec_without_tables(), &input);
        prop_assert_eq!(unread, read);
    }
}

/// Every element type with lengths bounded on both sides: `NA` holds `N0` to
/// `N9`, `RS` holds `R` at scales 0, 3, 18 and the default, `CP` is a
/// composite of typed components and `TX` holds `AN`, `ID`, `DT` and `TM`.
/// Each segment has a table that reads every element and component, so the
/// full spec parses every value and the spec without tables only validates.
const TYPED_SPEC: &str = r#"{"name":"typed",
    "loops":{"head":{"trigger":{"segment":"HD"},"segments":["NA","RS","CP","TX"],"end":"TR"}},
    "segments":{
        "NA":{"elements":{
            "1":{"name":"n0","type":"N0","min":2,"max":4},
            "2":{"name":"n1","type":"N1","min":2,"max":4},
            "3":{"name":"n2","type":"N2","min":2,"max":4},
            "4":{"name":"n3","type":"N3","min":2,"max":4},
            "5":{"name":"n4","type":"N4","min":2,"max":4},
            "6":{"name":"n5","type":"N5","min":2,"max":4},
            "7":{"name":"n6","type":"N6","min":2,"max":4},
            "8":{"name":"n7","type":"N7","min":2,"max":4},
            "9":{"name":"n8","type":"N8","min":2,"max":4},
            "10":{"name":"n9","type":"N9","min":2,"max":4}
        }},
        "RS":{"elements":{
            "1":{"name":"r0","type":"R","scale":0,"min":2,"max":4},
            "2":{"name":"r3","type":"R","scale":3,"min":2,"max":4},
            "3":{"name":"r18","type":"R","scale":18,"min":2,"max":4},
            "4":{"name":"r","type":"R","min":2,"max":4}
        }},
        "CP":{"elements":{
            "1":{"name":"mix","type":"AN","composite":{
                "1":{"name":"code","type":"ID","required":true,"min":2,"max":3},
                "2":{"name":"count","type":"N2","min":2,"max":4},
                "3":{"name":"amount","type":"R","scale":3,"min":2,"max":4},
                "4":{"name":"day","type":"DT"},
                "5":{"name":"time","type":"TM"},
                "6":{"name":"note","type":"AN","min":2,"max":4}
            }}
        }},
        "TX":{"elements":{
            "1":{"name":"text","type":"AN","required":true,"min":2,"max":4},
            "2":{"name":"code","type":"ID","min":2,"max":4},
            "3":{"name":"day","type":"DT","min":6,"max":8},
            "4":{"name":"time","type":"TM","min":4,"max":6}
        }}
    },
    "tables":{
        "na":{"loops":["head"],"segment":"NA","columns":{
            "n0":{"element":1},"n1":{"element":2},"n2":{"element":3},"n3":{"element":4},
            "n4":{"element":5},"n5":{"element":6},"n6":{"element":7},"n7":{"element":8},
            "n8":{"element":9},"n9":{"element":10}
        }},
        "rs":{"loops":["head"],"segment":"RS","columns":{
            "r0":{"element":1},"r3":{"element":2},"r18":{"element":3},"r":{"element":4}
        }},
        "cp":{"loops":["head"],"segment":"CP","columns":{
            "code":{"element":1,"component":1},"count":{"element":1,"component":2},
            "amount":{"element":1,"component":3},"day":{"element":1,"component":4},
            "time":{"element":1,"component":5},"note":{"element":1,"component":6}
        }},
        "tx":{"loops":["head"],"segment":"TX","columns":{
            "text":{"element":1},"code":{"element":2},"day":{"element":3},"time":{"element":4}
        }}
    }
}"#;

/// Projects `input` with the typed spec read by its tables and without them,
/// asserts both give the same diagnostics, and returns them.
fn typed_parity(input: &str) -> Vec<Diagnostic> {
    let read_spec = Spec::from_json(TYPED_SPEC).unwrap();
    let bare = without_tables(TYPED_SPEC);
    let (_, read) = project(&read_spec, input);
    let (_, unread) = project(&bare, input);
    assert_eq!(rendered(&unread), rendered(&read));
    assert_eq!(unread, read);
    read
}

/// How many diagnostics of each rule kind, at which element and component.
fn kinds(diagnostics: &[Diagnostic]) -> Vec<(&'static str, Option<usize>, Option<usize>)> {
    diagnostics
        .iter()
        .map(|d| (d.rule.kind(), d.element, d.component))
        .collect()
}

#[test]
fn the_typed_spec_reads_every_element_and_its_bare_copy_none() {
    let read_spec = Spec::from_json(TYPED_SPEC).unwrap();
    let projector = Projector::new(&read_spec, &delimiters());
    let bare = without_tables(TYPED_SPEC);
    let unread = Projector::new(&bare, &delimiters());
    // A composite's own flag does not matter: its components are checked one
    // by one, each with its own flag.
    let leaves_read = |(_, read, components): &Flags| {
        if components.is_empty() {
            *read
        } else {
            components.iter().all(|&(_, r)| r)
        }
    };
    for id in [&b"NA"[..], b"RS", b"CP", b"TX"] {
        let read_flags = flags(&projector, id);
        assert!(read_flags.iter().all(leaves_read), "{id:?}: {read_flags:?}");
        let unread_flags = flags(&unread, id);
        assert!(
            unread_flags
                .iter()
                .all(|(_, read, components)| !*read && components.iter().all(|&(_, r)| !r)),
            "{id:?}: {unread_flags:?}"
        );
    }
}

#[test]
fn implied_decimals_n0_to_n9_raise_the_same_diagnostics_read_or_unread() {
    // Per row: at min (2 digits), at max (4 digits, sign not counted), one
    // short, one long, then type mismatches.
    let input = "HD~\
        NA*12*-12*1234*-1234*01*99*1000*-9999*10*42~\
        NA*1*-1*12345*-12345*0*123456*7*-1*99999*5~\
        NA*1.2*+12*1 2*A*-*12-*--1*.5*1:2*ZZ~\
        TR~";
    let diagnostics = typed_parity(input);
    let found = kinds(&diagnostics);
    assert_eq!(
        found
            .iter()
            .filter(|(kind, ..)| *kind == "LengthOutOfRange")
            .count(),
        10
    );
    assert_eq!(
        found
            .iter()
            .filter(|(kind, ..)| *kind == "TypeMismatch")
            .count(),
        10
    );
    assert_eq!(found.len(), 20, "{:#?}", rendered(&diagnostics));
}

#[test]
fn r_with_a_custom_scale_raises_the_same_diagnostics_read_or_unread() {
    // Scale 0 refuses any fraction digit, scale 3 refuses a fourth one; digits
    // only count toward the length.
    let input = "HD~\
        RS*12*1.234*-12.34*12.~\
        RS*1234*-.123*12.34*-12.3~\
        RS*1*1.2345*1*.5~\
        RS*12345*12345*123.45*12345~\
        RS*1.5*1.2345*1.1234567890123456789*1.234~\
        RS*-*.*1e3*+1~\
        TR~";
    let diagnostics = typed_parity(input);
    let found = kinds(&diagnostics);
    // Row 3: three short values and 1.2345, which scale 3 does not parse.
    assert!(found.contains(&("LengthOutOfRange", Some(1), None)));
    assert!(found.contains(&("TypeMismatch", Some(2), None)));
    // Row 4: four long values; row 5: four type mismatches (too many decimals).
    assert_eq!(
        found
            .iter()
            .filter(|(kind, ..)| *kind == "LengthOutOfRange")
            .count(),
        3 + 4,
        "{:#?}",
        rendered(&diagnostics)
    );
    assert_eq!(
        found
            .iter()
            .filter(|(kind, ..)| *kind == "TypeMismatch")
            .count(),
        1 + 4 + 4,
        "{:#?}",
        rendered(&diagnostics)
    );
}

#[test]
fn typed_composite_components_raise_the_same_diagnostics_read_or_unread() {
    let input = "HD~\
        CP*AB:12:1.2:20240229:1230:ab~\
        CP*ABC:1234:12.34:240229:123059:abcd~\
        CP*A:1:1:20230229:2460:a~\
        CP*ABCD:12345:12.345:2024022:12:abcde~\
        CP*:1.5:1.2345:X:X~\
        CP*AB:12:12:20240101:1200:ab:EXTRA~\
        CP*~\
        TR~";
    let diagnostics = typed_parity(input);
    let found = kinds(&diagnostics);
    assert!(found.contains(&("RequiredElementMissing", Some(1), Some(1))));
    assert!(found.contains(&("CompositeShape", Some(1), Some(7))));
    for component in 1..=6 {
        assert!(
            found.contains(&("LengthOutOfRange", Some(1), Some(component)))
                || found.contains(&("TypeMismatch", Some(1), Some(component))),
            "component {component}: {:#?}",
            rendered(&diagnostics)
        );
    }
    assert!(found.len() >= 15, "{:#?}", rendered(&diagnostics));
}

#[test]
fn lengths_at_and_one_past_each_bound_raise_the_same_diagnostics_read_or_unread() {
    // AN and ID count bytes; DT is 6 or 8 by shape, TM 4 to 8.
    let input = "HD~\
        TX*ab*AB*240101*1230~\
        TX*abcd*ABCD*20240101*123045~\
        TX*a*A*240101*1230~\
        TX*abcde*ABCDE*20240101*1230451~\
        TX***~\
        TR~";
    let diagnostics = typed_parity(input);
    assert_eq!(
        kinds(&diagnostics),
        vec![
            ("LengthOutOfRange", Some(1), None),
            ("LengthOutOfRange", Some(2), None),
            ("LengthOutOfRange", Some(1), None),
            ("LengthOutOfRange", Some(2), None),
            ("LengthOutOfRange", Some(4), None),
            ("RequiredElementMissing", Some(1), None),
        ],
        "{:#?}",
        rendered(&diagnostics)
    );
}

proptest! {
    #[test]
    fn typed_diagnostics_do_not_depend_on_what_columns_read(
        values in proptest::collection::vec(
            proptest::collection::vec(
                proptest::sample::select(b"0159-.:A".to_vec()),
                0..8,
            ),
            20,
        ),
    ) {
        let text: Vec<String> = values
            .iter()
            .map(|value| String::from_utf8(value.clone()).unwrap())
            .collect();
        let input = format!(
            "HD~NA*{}~RS*{}~CP*{}~TX*{}~TR~",
            text[..10].join("*"),
            text[10..14].join("*"),
            text[14..17].join(":"),
            text[17..].join("*"),
        );
        let read_spec = Spec::from_json(TYPED_SPEC).unwrap();
        let (_, read) = project(&read_spec, &input);
        let (_, unread) = project(&without_tables(TYPED_SPEC), &input);
        prop_assert_eq!(unread, read);
    }
}
