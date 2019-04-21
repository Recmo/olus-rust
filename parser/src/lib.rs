#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
use nom::*;
pub use unic::normal::StrNormalForm;
use unic::ucd::category::GeneralCategory;
use unic::ucd::ident::{is_pattern_syntax, is_pattern_whitespace, is_xid_continue, is_xid_start};
pub use unic::UNICODE_VERSION;

// UAX14 Line terminators (Mandatory Break BK)
// @see https://www.unicode.org/reports/tr14/tr14-32.html
// @see https://www.unicode.org/versions/Unicode12.0.0/ch05.pdf
//      Section 5.8 "Newline Guidelines"

fn is_line_terminator(c: char) -> bool {
    match c {
        '\u{A}' => true,  // Line Feed
        '\u{B}' => true,  // Vertical Tab
        '\u{C}' => true,  // Form Feed
        '\u{D}' => true,  // Carriage Return (unless followed by Line Feed)
        '\u{85}' => true, // Next Line
        _ => match GeneralCategory::of(c) {
            GeneralCategory::LineSeparator => true,
            GeneralCategory::ParagraphSeparator => true,
            _ => false,
        },
    }
}

// Either a line terminator or Carriage Return + Line Feed.
named!(line_separator<&str, ()>, value!((), alt!(
    tag!("\u{D}\u{A}") | take_while_m_n!(1, 1, is_line_terminator)
)));

// NOM matchers for unicode UAX31
// @see https://www.unicode.org/reports/tr31/

named!(identifier<&str, &str>, recognize!(
    tuple!(take_while_m_n!(1, 1, is_xid_start), take_while!(is_xid_continue))
));

named!(syntax<&str, &str>, take_while_m_n!(1, 1, is_pattern_syntax));

named!(whitespace<&str, ()>, value!((), take_while!(is_pattern_whitespace)));

named!(whitespace_no_newline<&str, ()>, value!((), take_while!(|c|
    !is_line_terminator(c) && is_pattern_whitespace(c)
)));

//
// NOM matchers for strings and numbers
//

// https://unicode-table.com/en/sets/quotation-marks/
// Strings are quoted with English double quotes “ ”. Quotes can be nested.

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn test_is_line_terminator() {
        assert_eq!(is_line_terminator('\n'), true);
        assert_eq!(is_line_terminator('\r'), true);
        assert_eq!(is_line_terminator('\u{2028}'), true); // Line Separator
        assert_eq!(is_line_terminator('\u{2029}'), true); // Paragraph Separator
        assert_eq!(is_line_terminator('\t'), false);
        assert_eq!(is_line_terminator(' '), false);
    }

    #[test]
    fn parse_line_separator() {
        assert_eq!(line_separator(""), Err(Err::Incomplete(Needed::Size(2))));
        assert_eq!(
            line_separator("\u{D}"),
            Err(Err::Incomplete(Needed::Size(2)))
        );
        assert_eq!(line_separator(" ").is_err(), true);
        assert_eq!(line_separator("\n\t"), Ok(("\t", ())));
        assert_eq!(line_separator("\u{D}\u{A}\n"), Ok(("\n", ())));
        assert_eq!(line_separator("\u{D}a"), Ok(("a", ())));
        assert_eq!(line_separator("\u{A}\u{D}\n"), Ok(("\u{D}\n", ())));
    }

    #[test]
    fn parse_identifier() {
        assert_eq!(identifier("hello"), Err(Err::Incomplete(Needed::Size(1))));
        assert_eq!(identifier("hello "), Ok((" ", "hello")));
        assert_eq!(identifier("he_llo "), Ok((" ", "he_llo")));
        assert_eq!(identifier("he-llo "), Ok(("-llo ", "he")));
        assert_eq!(identifier("he≈llo "), Ok(("≈llo ", "he")));
        assert!(identifier("_hello ").is_err());
    }

    #[test]
    fn parse_whitespace() {
        assert_eq!(whitespace(""), Err(Err::Incomplete(Needed::Size(1))));
        assert_eq!(whitespace("a"), Ok(("a", ())));
        assert_eq!(whitespace(" a"), Ok(("a", ())));
        assert_eq!(whitespace(" \t\n\r a"), Ok(("a", ())));
    }

    #[test]
    fn parse_syntax() {
        assert_eq!(syntax("+ a"), Ok((" a", "+")));
        // TODO: assert_eq!(syntax("≈ a"), Ok((" a", "≈")));
    }

}
