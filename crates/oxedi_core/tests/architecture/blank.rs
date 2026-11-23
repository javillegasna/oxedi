//! Blanks the comments of a Rust source and the escapes of its literals, and leaves the rest of
//! each literal as written, so a path in a comment is not read as a use and a path in a string is.

pub fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// What a stretch of source is, for blanking comments.
enum Kind {
    Code,
    Comment,
    /// A raw string, written out as it is.
    Raw,
    /// A string or character literal, written out with each escape blanked.
    Quoted,
}

/// The text with every comment, and every escape of a string or character literal, replaced by
/// spaces, line breaks kept. Raw strings are left as written.
pub fn strip_comments(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut at = 0;
    while at < b.len() {
        let (len, kind) = token_at(b, at);
        let token = &b[at..at + len];
        match kind {
            Kind::Comment => blank_into(&mut out, token),
            Kind::Code | Kind::Raw => out.extend_from_slice(token),
            Kind::Quoted => quoted_into(&mut out, token),
        }
        at += len;
    }
    out
}

/// Writes `bytes` as spaces, line breaks kept.
fn blank_into(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(bytes.iter().map(|&c| if c == b'\n' { c } else { b' ' }));
}

/// Writes a string or character literal with each escape blanked, so a path right after an
/// escape (`"\ncrate::spec"`) starts after a space and is read whole.
fn quoted_into(out: &mut Vec<u8>, token: &[u8]) {
    let mut at = 0;
    while at < token.len() {
        let rest = &token[at..];
        if rest[0] == b'\\' {
            let len = escape_len(rest);
            blank_into(out, &rest[..len]);
            at += len;
        } else {
            out.push(rest[0]);
            at += 1;
        }
    }
}

/// Length of the escape at `rest[0]`: `\x41` is four bytes, `\u{1F600}` runs to its closing
/// brace, any other (`\n`, `\"`, a backslash before a line break) is two.
fn escape_len(rest: &[u8]) -> usize {
    let len = match rest.get(1) {
        Some(b'x') => 4,
        Some(b'u') => rest
            .iter()
            .position(|&c| c == b'}')
            .map_or(rest.len(), |p| p + 1),
        _ => 2,
    };
    len.min(rest.len())
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
        (len, Kind::Raw)
    } else if rest[0] == b'"' {
        (string_len(rest), Kind::Quoted)
    } else if rest[0] == b'\'' {
        (char_len(rest), Kind::Quoted)
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

/// Length of a raw string (`r"..."`, `r#"..."#`, `br"..."`, `cr#"..."#`) that starts at `b[at]`,
/// when one does.
fn raw_string_len(b: &[u8], at: usize) -> Option<usize> {
    let rest = &b[at..];
    if at > 0 && is_ident(b[at - 1]) {
        return None;
    }
    let prefix = usize::from(matches!(rest[0], b'b' | b'c'));
    if rest.get(prefix) != Some(&b'r') {
        return None;
    }
    let hashes = rest[prefix + 1..]
        .iter()
        .take_while(|&&c| c == b'#')
        .count();
    if rest.get(prefix + 1 + hashes) != Some(&b'"') {
        return None;
    }
    let mut closing = vec![b'"'];
    closing.resize(1 + hashes, b'#');
    let body = prefix + 2 + hashes;
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
    let width = rest.get(1).map_or(1, |&lead| utf8_width(lead));
    if rest.get(1 + width) == Some(&b'\'') {
        2 + width
    } else {
        1
    }
}

/// Bytes of the UTF-8 character whose first byte is `lead`.
fn utf8_width(lead: u8) -> usize {
    match lead.leading_ones() {
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 1,
    }
}
