pub(crate) const COLOR_NORMAL: &'static str = "\x1b[0m";
pub(crate) const COLOR_BLACK: &'static str = "\x1b[30;47m";
pub(crate) const COLOR_RED: &'static str = "\x1b[31;40m";
pub(crate) const COLOR_GREEN: &'static str = "\x1b[32;40m";
pub(crate) const COLOR_YELLOW: &'static str = "\x1b[33;40m";
pub(crate) const COLOR_BLUE: &'static str = "\x1b[34;40m";
pub(crate) const COLOR_MAGENTA: &'static str = "\x1b[35;40m";
pub(crate) const COLOR_CYAN: &'static str = "\x1b[36;40m";
pub(crate) const COLOR_WHITE: &'static str = "\x1b[37;40m";

pub(crate) const COLOR_BRIGHT_RED: &'static str = "\x1b[1;31;40m";
pub(crate) const COLOR_BRIGHT_GREEN: &'static str = "\x1b[1;32;40m";
pub(crate) const COLOR_BRIGHT_YELLOW: &'static str = "\x1b[1;33;40m";
pub(crate) const COLOR_BRIGHT_BLUE: &'static str = "\x1b[1;34;40m";
pub(crate) const COLOR_BRIGHT_MAGENTA: &'static str = "\x1b[1;35;40m";
pub(crate) const COLOR_BRIGHT_CYAN: &'static str = "\x1b[1;36;40m";
pub(crate) const COLOR_BRIGHT_WHITE: &'static str = "\x1b[1;37;40m";

/// A Category characterizes a group of debug messages. Categories can be enabled or disabled to
/// control logging for entire OS subsystems.
pub enum Category {
    /// Error messages, indicating that something has gone wrong.
    Error
}

impl Category {
    pub(crate) fn color(&self) -> &str {
        use self::Category::*;
        match self {
            &Error => COLOR_BRIGHT_RED
        }
    }

    /// Returns a human-friendly name for this debug category, such as `error` or `scheduler`.
    pub fn name(&self) -> &str {
        use self::Category::*;
        match self {
            &Error => "error"
        }
    }
}