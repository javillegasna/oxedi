use super::glob::is_pattern;

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
