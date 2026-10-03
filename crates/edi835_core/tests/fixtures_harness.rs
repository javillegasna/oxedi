//! Integration test that proves the fixture-loading pipeline works — the plumbing
//! the cross-layer (N7) tests will reuse. No parsing yet: Stage 0 only verifies the
//! fixtures are present and readable.

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
        // The core consumes bytes (P3), so the N7 plumbing must hand over bytes
        // untouched — never a `String` that would reject non-UTF-8 payer files.
        let content: Vec<u8> = common::load_fixture(name);
        assert!(!content.is_empty(), "fixture {name} is empty");
    }
}
