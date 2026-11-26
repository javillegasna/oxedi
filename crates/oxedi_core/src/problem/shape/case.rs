//! The shape of a conformance case fault.

use super::{Place, Problem};
use crate::conformance::CaseFault;

problem!(CaseFault [MissingFile, NotText, NotADocument] this => {
        Place::Kit {
            case: None,
            file: Some(this.path()),
        }
});
