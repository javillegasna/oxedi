//! The shape of a conformance case fault.

use super::{Place, Problem};
use crate::conformance::CaseFault;

problem!(CaseFault [MissingFile, NotText, NotADocument] this => {
        let (CaseFault::MissingFile { path }
        | CaseFault::NotText { path }
        | CaseFault::NotADocument { path, .. }) = this;
        Place::Kit {
            case: None,
            file: Some(path),
        }
});
