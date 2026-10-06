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

#[test]
fn a_file_no_longer_than_the_content_may_be_written_over() {
    assert_eq!(super::write::fits(0, 10, String::new), Ok(()));
    assert_eq!(super::write::fits(10, 10, String::new), Ok(()));
}

#[test]
fn a_longer_file_cannot_be_truncated() {
    assert_eq!(
        super::write::fits(11, 10, String::new),
        Err(super::write::WriteFailure {
            step: "opened for writing",
            message: "it already holds 11 bytes, more than the 10 to write, and DuckDB's file \
                      system cannot truncate a file from an extension; remove it first, or leave \
                      USE_TMP_FILE at its default"
                .to_owned(),
        })
    );
}

#[test]
fn a_size_duckdb_cannot_measure_reports_its_cause() {
    assert_eq!(
        super::write::fits(-1, 10, || "the file size is unknown".to_owned()),
        Err(super::write::WriteFailure {
            step: "measured",
            message: "the file size is unknown".to_owned(),
        })
    );
}
