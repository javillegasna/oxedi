//! Elements no column reads are validated without keeping their value; their
//! diagnostics must not depend on whether a column reads them.

use super::tests::{SPEC, delimiters, project, rendered, spec};
use super::*;
use proptest::prelude::*;

/// The test spec without its tables, so no column reads any element.
fn spec_without_tables() -> Spec {
    let mut json: serde_json::Value = serde_json::from_str(SPEC).unwrap();
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
