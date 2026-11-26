//! The shape of the checked values: a SNIP level, a segment index, a
//! delimiter byte and a float for a decimal column. None points at a place.

use super::{Place, Problem};
use crate::delimiters::DelimiterLength;
use crate::diagnostic::UnknownLevel;
use crate::document::SegmentOutOfRange;
use crate::value::FloatForDecimal;

problem!(UnknownLevel _this => Place::Nowhere);
problem!(SegmentOutOfRange _this => Place::Nowhere);
problem!(DelimiterLength _this => Place::Nowhere);
problem!(FloatForDecimal _this => Place::Nowhere);
