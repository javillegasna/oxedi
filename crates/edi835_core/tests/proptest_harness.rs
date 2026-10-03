//! Proves the `proptest` harness runs. In Stage 1 this file's pattern becomes
//! the real tokenize→reconstruct round-trip property.

use proptest::prelude::*;

proptest! {
    /// Reversing a string twice yields the original — a trivial involution,
    /// here only to exercise the property-test machinery end to end.
    #[test]
    fn string_reverse_is_involutive(s in ".*") {
        let reversed: String = s.chars().rev().collect();
        let back: String = reversed.chars().rev().collect();
        prop_assert_eq!(s, back);
    }
}
