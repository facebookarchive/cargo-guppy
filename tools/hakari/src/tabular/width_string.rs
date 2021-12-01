use std::fmt::{Debug, Formatter};

#[derive(Clone, Default)]
pub struct WidthString {
    string: String,
    width: usize,
}

impl WidthString {
    pub fn new<T: ToString>(thing: T) -> Self {
        let string = thing.to_string();
        let string_without_color = strip_ansi_escapes::strip(&string)
            .map(|str| String::from_utf8(str).unwrap_or_else(|_| thing.to_string()))
            .unwrap_or_else(|_| thing.to_string());
        #[cfg(feature = "unicode-width")]
        let width = ::unicode_width::UnicodeWidthStr::width(string_without_color.as_str());
        #[cfg(not(feature = "unicode-width"))]
        let width = string_without_color.chars().count();
        WidthString { string, width }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }
}

impl Debug for WidthString {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", self.string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owo_colors::OwoColorize;

    #[test]
    fn simple_string() {
        assert_eq!(WidthString::new("hello world").width, 11)
    }

    #[test]
    fn unicode_string() {
        assert_eq!(WidthString::new("unicode 🚀").width, 10)
    }

    #[test]
    fn colored_string() {
        assert_eq!(
            WidthString::new("hello world").width,
            WidthString::new("hello world".yellow()).width
        )
    }
}
