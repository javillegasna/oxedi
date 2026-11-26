//! The shape of a reading diagnostic.

use super::{Detail, Place, Problem};
use crate::diagnostic::Diagnostic;

impl Problem for Diagnostic {
    fn kind(&self) -> &'static str {
        self.rule.kind()
    }

    fn place(&self) -> Place<'_> {
        Place::File {
            segment: self.segment,
            element: self.element,
            component: self.component,
            path: &self.path,
        }
    }

    fn detail(&self) -> Detail<'_> {
        Detail::Diagnostic {
            level: self.level,
            rule: &self.rule,
            datum: &self.datum,
            origin: self.origin(),
            code: self.code(),
        }
    }
}
