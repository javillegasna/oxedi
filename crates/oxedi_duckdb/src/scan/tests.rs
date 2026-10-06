use super::write::{cell_bytes, hugeint};

#[test]
fn hugeint_splits_low_and_high_words() {
    for value in [0i128, 1, -1, 25_264_000, i128::MAX, i128::MIN] {
        let split = hugeint(value);
        let joined = (i128::from(split.upper) << 64) | i128::from(split.lower);
        assert_eq!(joined, value);
    }
}

#[test]
fn cell_bytes_reads_between_offsets() {
    let offsets = [0, 3, 3, 5];
    let bytes = b"abcde";
    assert_eq!(cell_bytes(&offsets, bytes, 0), Some(&b"abc"[..]));
    assert_eq!(cell_bytes(&offsets, bytes, 1), Some(&b""[..]));
    assert_eq!(cell_bytes(&offsets, bytes, 2), Some(&b"de"[..]));
    assert_eq!(cell_bytes(&offsets, bytes, 3), None);
}

#[test]
fn cell_bytes_refuses_offsets_past_the_data() {
    assert_eq!(cell_bytes(&[0, 9], b"abc", 0), None);
    assert_eq!(cell_bytes(&[-1, 2], b"abc", 0), None);
}
