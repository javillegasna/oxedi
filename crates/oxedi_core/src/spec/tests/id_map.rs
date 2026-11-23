//! The map keyed by segment id.

use crate::spec::IdMap;

#[test]
fn a_seven_byte_id_and_an_eight_byte_id_sharing_its_bytes_are_two_entries() {
    let mut map = IdMap::default();
    *map.entry_or_insert_with(b"ABCDEFG", || 1) += 10;
    *map.entry_or_insert_with(b"ABCDEFGH", || 2) += 20;
    assert_eq!(map.get(b"ABCDEFG"), Some(&11));
    assert_eq!(map.get(b"ABCDEFGH"), Some(&22));
    assert_eq!(map.get_mut(b"ABCDEFGH").map(|value| *value), Some(22));
    assert_eq!(map.get(b"ABCDEF"), None);
    assert_eq!(map.get(b"ABCDEFGHI"), None);
}
