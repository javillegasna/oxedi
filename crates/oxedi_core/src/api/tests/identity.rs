use crate::api::{Identity, builtin};
use crate::spec_load::{Builtin, Version};

#[test]
fn fallback_is_the_registrys_first() {
    assert_eq!(Identity::fallback(), Identity::of(Builtin::fallback()));
    assert!(std::ptr::eq(
        builtin(Identity::fallback()).unwrap(),
        Builtin::fallback()
    ));
}

#[test]
fn the_default_version_is_the_fallbacks() {
    assert_eq!(Identity::fallback().version, Builtin::DEFAULT_VERSION);
}

#[test]
fn at_finds_the_fallbacks_set_at_a_version() {
    let found = builtin(Identity::at(Version::V4010)).unwrap();
    assert_eq!(found.version, Version::V4010);
}

#[test]
fn an_unknown_set_is_unknown_version() {
    let identity = Identity {
        transaction: "837",
        version: Version::V5010,
    };
    assert_eq!(
        builtin(identity).unwrap_err().to_string(),
        r#"unknown version "5010"; no built-in spec covers transaction "837""#
    );
}
