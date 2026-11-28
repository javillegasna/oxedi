//! The shape of the checked values: a SNIP level, a segment index, a
//! delimiter byte and a float for a decimal column point at no place; an
//! envelope date or time points at its envelope field.

use super::{Place, Problem};
use crate::delimiters::DelimiterLength;
use crate::diagnostic::UnknownLevel;
use crate::document::SegmentOutOfRange;
use crate::spec::EnvelopeField;
use crate::value::FloatForDecimal;
use crate::write::{EnvelopeSource, EnvelopeValueError};

problem!(UnknownLevel _this => Place::Nowhere);
problem!(SegmentOutOfRange _this => Place::Nowhere);
problem!(DelimiterLength _this => Place::Nowhere);
problem!(FloatForDecimal _this => Place::Nowhere);

problem!(EnvelopeValueError [NotADate, FractionalTime, TimeOutOfRange, TicksBeforeMidnight] this => {
    let field = match this {
        EnvelopeValueError::NotADate { .. } => EnvelopeField::Date,
        EnvelopeValueError::FractionalTime { .. }
        | EnvelopeValueError::TimeOutOfRange { .. }
        | EnvelopeValueError::TicksBeforeMidnight { .. } => EnvelopeField::Time,
    };
    Place::Envelope { field: EnvelopeSource::Caller(field) }
});
