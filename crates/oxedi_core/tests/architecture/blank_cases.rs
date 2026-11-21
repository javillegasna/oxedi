//! Cases of the blanker: what it keeps and what it blanks.

use super::blank::strip_comments;

#[test]
fn comments_become_spaces_and_literals_stay() {
    let cases = [
        ("a line comment", "a // b\nc", "a     \nc"),
        (
            "a doc comment",
            "/// [`X`]\nfn f() {}",
            "         \nfn f() {}",
        ),
        (
            "a block comment keeps its line breaks",
            "a /* b\nc */ d",
            "a     \n     d",
        ),
        (
            "a nested block comment",
            "/* a /* b */ c */x",
            "                 x",
        ),
        (
            "a comment marker in a string",
            "\"a // b\" c",
            "\"a // b\" c",
        ),
        (
            "an escaped quote in a string",
            "\"a\\\" // b\" c",
            "\"a\\\" // b\" c",
        ),
        (
            "a comment marker in a raw string",
            "r#\"a // \"b\"# c",
            "r#\"a // \"b\"# c",
        ),
        ("a quote in a character literal", "'\"' // b", "'\"'     "),
        (
            "a lifetime is not a literal",
            "&'a str // b",
            "&'a str     ",
        ),
    ];
    for (name, text, expected) in cases {
        let got = String::from_utf8(strip_comments(text)).expect("blanking keeps UTF-8");
        assert_eq!(got, expected, "case {name}");
    }
}
