//! Cases of the blanker: what it keeps and what it blanks.

use super::blank::blank_comments_and_escapes;

#[test]
fn comments_and_escapes_become_spaces_and_literals_stay() {
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
            "\"a   // b\" c",
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
        ("a newline escape", "\"\\ncrate\"", "\"  crate\""),
        ("a byte escape", "\"\\x41y\"", "\"    y\""),
        ("a unicode escape", "\"\\u{41}z\"", "\"      z\""),
        (
            "an escaped line break keeps its line",
            "\"a\\\nb\" // c",
            "\"a \nb\"     ",
        ),
        (
            "an escaped quote in a character literal",
            "'\\'' // b",
            "'  '     ",
        ),
        (
            "a raw string keeps its backslashes",
            "r\"\\n\" c",
            "r\"\\n\" c",
        ),
        ("a raw byte string", "br\"\\\" // b\" c", "br\"\\\"        "),
        (
            "a raw C string with hashes",
            "cr#\"a\"# // b",
            "cr#\"a\"#     ",
        ),
        (
            "a non-ASCII character literal",
            "['é','\"'] // b",
            "['é','\"']     ",
        ),
    ];
    for (name, text, expected) in cases {
        let got =
            String::from_utf8(blank_comments_and_escapes(text)).expect("blanking keeps UTF-8");
        assert_eq!(got, expected, "case {name}");
    }
}
