//! Proves the fixture-loading pipeline works: the fixtures are present and
//! readable as bytes.

mod common;

#[test]
fn all_fixtures_load_and_are_nonempty() {
    let fixtures = [
        "blue_cross_nc_sample.txt",
        "united_healthcare_legacy_sample.txt",
        "trizetto_sample.rmt",
        "emedny_sample.txt",
        "multi_claim_sample.txt",
    ];
    for name in fixtures {
        // Bytes, never a `String`: a `String` would reject non-UTF-8 payer files.
        let content: Vec<u8> = common::load_fixture(name);
        assert!(!content.is_empty(), "fixture {name} is empty");
    }
}
