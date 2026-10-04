//! Every sample and fixture gets the built-in spec of the version its group
//! header declares, and the 5010 spec when it declares none the specs know.

mod common;

use edi835_core::{Document, Spec};

fn selected(bytes: Vec<u8>) -> String {
    let five = Spec::builtin_835();
    let four = Spec::builtin_835_4010();
    let document = Document::parse(bytes).unwrap();
    let chosen = Spec::select(&[&five, &four], &five, document.segments());
    if std::ptr::eq(chosen, &four) {
        "4010".into()
    } else {
        assert!(std::ptr::eq(chosen, &five));
        "5010".into()
    }
}

#[test]
fn samples_get_the_spec_of_their_declared_version() {
    let cases = [
        ("edi835_test_davisvision.RMT", "4010"),
        ("edi835_test_eyemed.RMT", "4010"),
        ("edi835_test_file.RMT", "4010"),
        ("edi835_test_not_available_claim_id.RMT", "4010"),
        ("edi835_test_versant.RMT", "4010"),
        ("edi835_test_united.rmt", "5010"),
    ];
    for (name, expected) in cases {
        assert_eq!(selected(common::load_sample(name)), expected, "{name}");
    }
}

#[test]
fn fixtures_get_the_spec_of_their_declared_version() {
    // `004010` alone (no implementation guide) is still the 4010 835.
    let cases = [
        ("emedny_sample.txt", "5010"),
        ("united_healthcare_legacy_sample.txt", "5010"),
        ("multi_claim_sample.txt", "4010"),
        ("trizetto_sample.rmt", "4010"),
    ];
    for (name, expected) in cases {
        assert_eq!(selected(common::load_fixture(name)), expected, "{name}");
    }
}
