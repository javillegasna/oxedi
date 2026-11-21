//! Blanks the comments of a Rust source and leaves its literals as written, so a path in a
//! comment is not read as a use and a path in a string is.

pub fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// What a stretch of source is, for blanking comments.
enum Kind {
    Code,
    Comment,
    Literal,
}

/// The text with every comment replaced by spaces, line breaks kept. String and character
/// literals are left as written.
pub fn strip_comments(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut at = 0;
    while at < b.len() {
        let (len, kind) = token_at(b, at);
        let token = &b[at..at + len];
        match kind {
            Kind::Comment => out.extend(token.iter().map(|&c| if c == b'\n' { c } else { b' ' })),
            Kind::Code | Kind::Literal => out.extend_from_slice(token),
        }
        at += len;
    }
    out
}

fn token_at(b: &[u8], at: usize) -> (usize, Kind) {
    let rest = &b[at..];
    if rest.starts_with(b"//") {
        (
            rest.iter().position(|&c| c == b'\n').unwrap_or(rest.len()),
            Kind::Comment,
        )
    } else if rest.starts_with(b"/*") {
        (block_comment_len(rest), Kind::Comment)
    } else if let Some(len) = raw_string_len(b, at) {
        (len, Kind::Literal)
    } else if rest[0] == b'"' {
        (string_len(rest), Kind::Literal)
    } else if rest[0] == b'\'' {
        (char_len(rest), Kind::Literal)
    } else {
        (1, Kind::Code)
    }
}

/// Length of a block comment, which nests in Rust.
fn block_comment_len(rest: &[u8]) -> usize {
    let mut depth = 0;
    let mut i = 0;
    while i < rest.len() {
        if rest[i..].starts_with(b"/*") {
            depth += 1;
            i += 2;
        } else if rest[i..].starts_with(b"*/") {
            depth -= 1;
            i += 2;
        } else {
            i += 1;
        }
        if depth == 0 {
            return i;
        }
    }
    rest.len()
}

/// Length of a `"..."` literal that starts at `rest[0]`, escapes honoured.
fn string_len(rest: &[u8]) -> usize {
    let mut i = 1;
    while i < rest.len() {
        match rest[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    rest.len()
}

/// Length of a raw string (`r"..."`, `r#"..."#`) that starts at `b[at]`, when one does.
fn raw_string_len(b: &[u8], at: usize) -> Option<usize> {
    let rest = &b[at..];
    if rest[0] != b'r' || (at > 0 && is_ident(b[at - 1])) {
        return None;
    }
    let hashes = rest[1..].iter().take_while(|&&c| c == b'#').count();
    if rest.get(1 + hashes) != Some(&b'"') {
        return None;
    }
    let mut closing = vec![b'"'];
    closing.resize(1 + hashes, b'#');
    let body = 2 + hashes;
    let end = rest[body..]
        .windows(closing.len())
        .position(|w| w == closing.as_slice());
    Some(end.map_or(rest.len(), |p| body + p + closing.len()))
}

/// Length of a character literal at `rest[0]`; a lone `'` (a lifetime) is one byte.
fn char_len(rest: &[u8]) -> usize {
    if rest.get(1) == Some(&b'\\') {
        let close = rest.iter().skip(3).position(|&c| c == b'\'');
        return close.map_or(1, |p| p + 4);
    }
    if rest.get(2) == Some(&b'\'') { 3 } else { 1 }
}
