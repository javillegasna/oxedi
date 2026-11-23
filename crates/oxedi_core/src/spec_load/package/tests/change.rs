use super::super::change::{Change, ChangeKind};
use super::super::*;

#[test]
fn change_display() {
    let package = PackageId::parse("test/dictionary/005010").unwrap();
    let cases = [
        (ChangeKind::Removed, "x is removed"),
        (
            ChangeKind::Narrowed {
                before: "1".into(),
                after: "2".into(),
            },
            "x: 1 becomes 2",
        ),
        (
            ChangeKind::Widened {
                before: "2".into(),
                after: "3".into(),
            },
            "x: 2 becomes 3",
        ),
        (
            ChangeKind::Changed {
                before: "a".into(),
                after: "b".into(),
            },
            "x: a becomes b",
        ),
    ];
    for (kind, text) in cases {
        let path = "x".to_owned();
        let change = Change {
            package: package.clone(),
            path,
            kind,
        };
        assert_eq!(change.to_string(), text);
    }
}
