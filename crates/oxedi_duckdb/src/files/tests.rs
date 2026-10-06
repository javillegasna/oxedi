use super::glob::{file_name, is_pattern};

#[test]
fn patterns_are_paths_with_glob_characters() {
    assert!(is_pattern("data/*.835"));
    assert!(is_pattern("s3://bucket/file?.rmt"));
    assert!(is_pattern("data/[ab].835"));
    assert!(!is_pattern("data/file.835"));
    assert!(!is_pattern("https://example.com/remit.835"));
}

#[test]
fn http_urls_are_never_patterns() {
    assert!(!is_pattern("https://example.com/remit.835?sig=a*b"));
    assert!(!is_pattern("HTTP://example.com/[x].835"));
    assert!(is_pattern("s3://bucket/remit?.835"));
}

#[test]
fn file_names_must_be_utf8() {
    assert_eq!(file_name(b"data/a.835"), Ok("data/a.835".to_owned()));
    assert_eq!(
        file_name(b"data/M\xfcller.835"),
        Err("a matched file name is not valid UTF-8: b\"data/M\\xfcller.835\"".to_owned())
    );
}
