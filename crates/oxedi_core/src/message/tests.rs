use super::*;

#[test]
fn element_ref_names_an_element_and_a_component() {
    assert_eq!(ElementRef::simple(b"CLP", 2).to_string(), "CLP02");
    assert_eq!(ElementRef::new(b"SVC", 1, Some(2)).to_string(), "SVC01-2");
    assert_eq!(ElementRef::simple(b"N1", 10).to_string(), "N110");
}

#[test]
fn element_ref_reads_bytes_lossily() {
    assert_eq!(ElementRef::simple(b"C\xFFP", 1).to_string(), "C\u{FFFD}P01");
}
