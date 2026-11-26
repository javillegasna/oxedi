//! The dependencies between the core's modules equal the table in `table.rs`: a module uses
//! only the modules its row lists (or an exception names), every listed one is used, and the
//! rows hold no cycle. A new dependency fails here until the table says it is allowed.

mod blank;
mod blank_cases;
mod files;
mod scan;
mod scan_cases;
mod table;
mod text;
mod text_cases;
mod violations;
mod violations_cases;

use table::{EXCEPTIONS, NOTATION, RENDERERS, TABLE};
use violations::violations;

#[test]
fn the_core_uses_only_what_the_table_allows() {
    let src = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let modules = files::modules_of(src);
    let found = violations(&modules, &files::core_uses(src), TABLE, EXCEPTIONS);
    let lines: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert!(found.is_empty(), "\n{}\n", lines.join("\n"));
}

#[test]
fn every_exception_says_why() {
    for e in EXCEPTIONS {
        assert!(
            !e.reason.is_empty(),
            "the exception {} -> {} has no reason",
            e.from,
            e.to
        );
    }
}

#[test]
fn problem_text_is_written_only_in_problem() {
    let src = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let (displays, renderers) = text::text_finds(src);
    let found = text::text_violations(displays, renderers, NOTATION, RENDERERS);
    let lines: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert!(found.is_empty(), "\n{}\n", lines.join("\n"));
}

#[test]
fn every_listed_name_says_why() {
    for l in NOTATION.iter().chain(RENDERERS) {
        assert!(!l.reason.is_empty(), "{} has no reason", l.name);
    }
}
