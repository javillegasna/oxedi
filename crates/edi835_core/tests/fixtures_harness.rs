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
        let content = common::load_fixture(name);
        assert!(!content.trim().is_empty(), "fixture {name} is empty");
    }
}
