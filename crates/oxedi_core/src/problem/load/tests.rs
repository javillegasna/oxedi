#[test]
fn declared_occurrences_are_listed_or_none_is_said() {
    assert_eq!(Declared(&[]).to_string(), "it declares none");
    let known = ["a".to_string(), "b".to_string()];
    assert_eq!(Declared(&known).to_string(), "it declares \"a\", \"b\"");
}
