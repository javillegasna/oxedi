impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::ZeroPosition { key } => {
                write!(f, "{key:?} must be a 1-based element position; found 0")
            }
            ControlError::NoEnd => write!(f, "the loop has no \"end\" segment to check"),
            ControlError::FieldOnOpener { element, field } => write!(
                f,
                "\"fields\" puts {:?} on element {element}, which holds the control number (\"opener_element\"); move the field to another element",
                field.word()
            ),
            ControlError::FieldOnHeaderPlace {
                element,
                place,
                field,
                segment,
            } => write!(
                f,
                "\"fields\" puts {:?} on element {element} of the top loop's trigger {segment:?}, the place of the {place}, which the syntax layer owns; move the field to another element",
                field.word()
            ),
            ControlError::FieldNotOnTrigger {
                element,
                field,
                segment,
            } => write!(
                f,
                "\"fields\" puts {:?} on element {element}, which the trigger segment {segment:?} does not define; define the element in \"segments\" or move the field",
                field.word()
            ),
            ControlError::FieldTwice {
                field,
                first,
                second,
            } => write!(
                f,
                "\"fields\" puts {:?} on elements {first} and {second}; a field goes on one element",
                field.word()
            ),
        }
    }
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::ZeroElement => write!(f, "\"element\" is 0; positions are 1-based"),
            VersionError::NoValues => write!(
                f,
                "\"values\" is empty; list at least one value the element may hold"
            ),
            VersionError::NoCodes { segment, element } => write!(
                f,
                "\"values\" is absent and element {element} of segment {segment:?} is not defined or lists no codes to take them from; write \"values\", or give segments.{segment}.elements.{element} its \"codes\""
            ),
            VersionError::EmptyValue { index } => write!(f, "\"values[{index}]\" is empty"),
            VersionError::DuplicateValue {
                value,
                first,
                second,
            } => write!(
                f,
                "value {value:?} is listed twice, at values[{first}] and values[{second}]"
            ),
        }
    }
}
