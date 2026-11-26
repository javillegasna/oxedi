use super::text::{Conditions, Counted, render_names};

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

#[test]
fn counted_writes_the_singular_for_one_and_the_plural_otherwise() {
    let count = |count| Counted(count, "byte", "bytes").to_string();
    assert_eq!(count(0), "0 bytes");
    assert_eq!(count(1), "1 byte");
    assert_eq!(count(2), "2 bytes");
}
