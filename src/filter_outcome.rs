use std::fmt::Display;
use termcolor::{Color, ColorSpec};

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum FilterOutcome {
    RequestDenied,
    ResponseDenied,
    Passed,
}

impl FilterOutcome {
    pub fn get_colour(&self) -> ColorSpec {
        let mut colour_spec = ColorSpec::new();

        let foreground = match self {
            FilterOutcome::RequestDenied => Color::Red,
            FilterOutcome::ResponseDenied => Color::Yellow,
            FilterOutcome::Passed => Color::Green,
        };

        colour_spec.set_fg(Some(foreground));

        colour_spec
    }
}

impl Display for FilterOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterOutcome::RequestDenied => write!(f, "REQUEST DENIED"),
            FilterOutcome::ResponseDenied => write!(f, "RESPONSE DENIED"),
            FilterOutcome::Passed => write!(f, "PASSED"),
        }
    }
}
