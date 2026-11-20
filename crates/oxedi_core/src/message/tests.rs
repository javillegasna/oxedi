use super::*;

#[test]
fn element_ref_names_an_element_and_a_component() {
    assert_eq!(ElementRef::simple(b"CLP", 2).to_string(), "CLP02");
    assert_eq!(ElementRef::new(b"SVC", 1, Some(2)).to_string(), "SVC01-2");
    assert_eq!(ElementRef::simple(b"N1", 10).to_string(), "N110");
    assert_eq!(place("CLP", 2, None), "CLP02");
    assert_eq!(place(&b"SVC"[..], 1, Some(2)), "SVC01-2");
}

#[test]
fn element_ref_reads_bytes_lossily() {
    assert_eq!(ElementRef::simple(b"C\xFFP", 1).to_string(), "C\u{FFFD}P01");
}

#[test]
fn conditions_render_as_a_where_clause() {
    assert_eq!(Conditions::<Vec<u8>>(&[]).to_string(), "");
    let bytes = [(1, b"PR".to_vec()), (2, b"X".to_vec())];
    assert_eq!(
        Conditions(&bytes).to_string(),
        " where {1: \"PR\", 2: \"X\"}"
    );
    let strings = [(1, "PR".to_string()), (2, "X".to_string())];
    assert_eq!(
        Conditions(&strings).to_string(),
        " where {1: \"PR\", 2: \"X\"}"
    );
}

#[test]
fn conditions_read_values_lossily() {
    assert_eq!(
        Conditions(&[(1, vec![0xFF])]).to_string(),
        " where {1: \"\u{FFFD}\"}"
    );
}

#[test]
fn names_are_quoted_and_joined() {
    assert_eq!(render_names::<&str>(&[]), "");
    assert_eq!(render_names(&["a", "b c"]), "\"a\", \"b c\"");
    assert_eq!(render_names(&["x".to_string()]), "\"x\"");
}
