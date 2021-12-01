use std::fmt::{Debug, Formatter};

#[derive(Clone, Default)]
pub struct WidthString {
    string: String,
    width: usize,
}

impl WidthString {
    pub fn new<T: ToString>(thing: T) -> Self {
        let string = thing.to_string();
        let width = Self::compute_width(&string);
        WidthString { string, width }
    }

    pub fn new_ansi<T: ToString>(thing: T) -> Self {
        let string = thing.to_string();
        let stripped_bytes =
            strip_ansi_escapes::strip(&string).expect("writing to a Cursor<Vec<u8>> is infallible");
        let stripped_string = String::from_utf8(stripped_bytes)
            .expect("a UTF-8 string with ANSI escapes stripped is valid UTF-8");
        let width = Self::compute_width(&stripped_string);
        WidthString { string, width }
    }

    pub fn custom_width<T: ToString>(thing: T, width: usize) -> Self {
        WidthString {
            string: thing.to_string(),
            width,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }

    #[cfg(feature = "unicode-width")]
    fn compute_width(s: &str) -> usize {
        unicode_width::UnicodeWidthStr::width(s)
    }

    #[cfg(not(feature = "unicode-width"))]
    fn compute_width(s: &str) -> usize {
        s.chars().count()
    }
}

impl Debug for WidthString {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", self.string)
    }
}
