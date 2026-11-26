impl fmt::Display for SnipLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SNIP {}", self.number())
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = Location {
            segment: self.segment,
            element: self.element,
            component: self.component,
        };
        write!(
            f,
            "{} · {} · {location} · at {} · datum {}",
            self.level,
            self.rule,
            Within(&self.path),
            Quoted(&self.datum)
        )
    }
}
