//! The `Display` text of every rule, one sentence naming the element, its
//! name and the offending value.

use std::fmt;

use super::{Quoted, Rule};

/// An element reference in X12 style: `CLP01`, or `SVC01-2` for a component.
struct ElementRef<'a> {
    segment_id: &'a [u8],
    element: usize,
    component: Option<usize>,
}

impl fmt::Display for ElementRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:02}",
            String::from_utf8_lossy(self.segment_id),
            self.element
        )?;
        match self.component {
            Some(component) => write!(f, "-{component}"),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::ByteOrderMark => write!(
                f,
                "the input starts with a UTF-8 byte order mark, kept as leading trivia of the first segment"
            ),
            Rule::UnknownSegment { id } => write!(
                f,
                "segment {} is not part of the structure: no open loop holds it and it opens no loop",
                Quoted(id)
            ),
            Rule::ImplicitLoop {
                loop_name,
                expected_trigger,
                caused_by,
            } => write!(
                f,
                "loop {loop_name:?} opened without its own trigger ({expected_trigger}) to hold segment {}",
                Quoted(caused_by)
            ),
            Rule::UnterminatedLoop {
                loop_name,
                expected_end,
                opened_at,
            } => {
                write!(f, "loop {loop_name:?} ")?;
                if let Some(opened_at) = opened_at {
                    write!(f, "opened at segment #{opened_at} ")?;
                }
                write!(f, "closed without its end segment {}", Quoted(expected_end))
            }
            Rule::ControlCountMismatch {
                segment_id,
                element,
                expected,
                found,
            } => write!(
                f,
                "{} declares {} but the count is {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                },
                Quoted(found)
            ),
            Rule::ControlElementMissing {
                segment_id,
                element,
            } => write!(
                f,
                "control element {} is missing: the segment has no element {element}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                }
            ),
            Rule::ControlNumberMismatch {
                opener,
                opener_element,
                closer,
                closer_element,
                opener_value,
                closer_value,
                opened_at,
            } => {
                write!(
                    f,
                    "{} {} does not match {} {}",
                    ElementRef {
                        segment_id: closer,
                        element: *closer_element,
                        component: None
                    },
                    Quoted(closer_value),
                    ElementRef {
                        segment_id: opener,
                        element: *opener_element,
                        component: None
                    },
                    Quoted(opener_value)
                )?;
                match opened_at {
                    Some(opened_at) => write!(f, " of segment #{opened_at}"),
                    None => Ok(()),
                }
            }
            Rule::RequiredElementMissing {
                segment_id,
                element,
                component,
                name,
            } => write!(
                f,
                "required element {} ({name}) is missing or empty",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::TypeMismatch {
                segment_id,
                element,
                component,
                name,
                expected,
            } => write!(
                f,
                "element {} ({name}) is not a valid {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::LengthOutOfRange {
                segment_id,
                element,
                component,
                name,
                min,
                max,
                length,
            } => {
                write!(
                    f,
                    "element {} ({name}) has length {length}; the spec allows ",
                    ElementRef {
                        segment_id,
                        element: *element,
                        component: *component
                    }
                )?;
                match (min, max) {
                    (Some(min), Some(max)) => write!(f, "{min} to {max}"),
                    (Some(min), None) => write!(f, "at least {min}"),
                    (None, Some(max)) => write!(f, "at most {max}"),
                    (None, None) => write!(f, "any length"),
                }
            }
            Rule::ValueDropped {
                table,
                column,
                bytes,
            } => write!(
                f,
                "text for column {column:?} of table {table:?} was not stored: it would bring the column to {bytes} bytes and a column holds at most {}",
                i32::MAX
            ),
            Rule::CompositeShape {
                segment_id,
                element,
                name,
                declared,
                found,
            } => write!(
                f,
                "element {} ({name}) has {found} components; the spec declares {declared}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                }
            ),
            Rule::CodeNotInList {
                segment_id,
                element,
                component,
                name,
                codes,
            } => {
                let at = ElementRef {
                    segment_id,
                    element: *element,
                    component: *component,
                };
                match codes {
                    1 => write!(
                        f,
                        "element {at} ({name}) is not the one code the spec lists for it"
                    ),
                    _ => write!(
                        f,
                        "element {at} ({name}) is not one of the {codes} codes the spec lists for it"
                    ),
                }
            }
            Rule::External {
                origin,
                code,
                message,
                ..
            } => match code {
                Some(code) => write!(f, "{message} (reported by {origin}, code {code})"),
                None => write!(f, "{message} (reported by {origin})"),
            },
        }
    }
}
