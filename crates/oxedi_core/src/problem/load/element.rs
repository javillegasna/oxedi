impl fmt::Display for ElementDefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementDefError::EmptyName => write!(f, "\"name\" is empty"),
            ElementDefError::DuplicateName { name, first } => {
                write!(f, "name {name:?} is already used by position {first:?}")
            }
            ElementDefError::MinAboveMax { min, max } => {
                write!(f, "\"min\" {min} is greater than \"max\" {max}")
            }
            ElementDefError::CompositeOnNonAn { kind } => {
                write!(f, "\"composite\" requires type AN; found type {kind:?}")
            }
            ElementDefError::NestedComposite => {
                write!(f, "a component cannot declare its own \"composite\"")
            }
            ElementDefError::ScaleAboveMaximum { scale } => write!(
                f,
                "\"scale\" {scale} is above the maximum of {}",
                ElementType::MAX_SCALE
            ),
            ElementDefError::ZeroMax => {
                write!(f, "\"max\" is 0; an element holds at least one character")
            }
            ElementDefError::EmptyCodes => write!(
                f,
                "\"codes\" is empty; leave the key out to accept any value"
            ),
            ElementDefError::EmptyCode { index } => {
                write!(f, "the code at codes[{index}] is empty")
            }
            ElementDefError::CodeLength {
                index,
                code,
                length,
                allowed,
            } => write!(
                f,
                "code {code:?} at codes[{index}] has length {length}; the element allows {allowed}"
            ),
            ElementDefError::DuplicateCode {
                code,
                first,
                second,
            } => write!(
                f,
                "code {code:?} is listed twice, at codes[{first}] and codes[{second}]"
            ),
            ElementDefError::CodesOnComposite => write!(
                f,
                "\"codes\" applies to a simple element or a component; this element declares a \"composite\""
            ),
            ElementDefError::CodesOnNumeric { kind } => write!(
                f,
                "\"codes\" applies to non-numeric types; type {kind:?} holds numbers, which one value can write several ways (1, 01, 1.0)"
            ),
        }
    }
}
