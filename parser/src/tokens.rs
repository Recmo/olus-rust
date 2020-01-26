#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
use nom::*;
use std::str::FromStr;
use unic::ucd::{
    category::GeneralCategory,
    ident::{is_pattern_syntax, is_pattern_whitespace, is_xid_continue, is_xid_start},
};

// UAX14 Line terminators (Mandatory Break BK)
// @see https://www.unicode.org/reports/tr14/tr14-32.html
// @see https://www.unicode.org/versions/Unicode12.0.0/ch05.pdf
//      Section 5.8 "Newline Guidelines"

pub(crate) fn is_line_terminator(c: char) -> bool {
    match c {
        '\u{A}' => true,  // Line Feed
        '\u{B}' => true,  // Vertical Tab
        '\u{C}' => true,  // Form Feed
        '\u{D}' => true,  // Carriage Return (unless followed by Line Feed)
        '\u{85}' => true, // Next Line
        _ => {
            match GeneralCategory::of(c) {
                GeneralCategory::LineSeparator => true,
                GeneralCategory::ParagraphSeparator => true,
                _ => false,
            }
        }
    }
}

// Unicode aware version of takewhile

pub(crate) fn take_char<F>(pred: F) -> impl Fn(&str) -> IResult<&str, &str>
where
    F: Fn(char) -> bool,
{
    move |input: &str| {
        match input.chars().next() {
            Some(c) => {
                if pred(c) {
                    let l = c.len_utf8();
                    Ok((&input[l..], &input[..l]))
                } else {
                    Err(Err::Error(error_position!(input, ErrorKind::Char)))
                }
            }
            None => Err(Err::Incomplete(Needed::Unknown)),
        }
    }
}

// Either a line terminator or Carriage Return + Line Feed.
named!(pub(crate) line_separator<&str, ()>, value!((), alt!( // TODO: alt_complete?
     tag!("\u{D}\u{A}") | map!(take_char(is_line_terminator), |a| a )
)));

// NOM matchers for unicode UAX31
// @see https://www.unicode.org/reports/tr31/

named!(pub(crate) identifier<&str, &str>, recognize!(
    tuple!(map!(take_char(is_xid_start), |a| a), take_while!(is_xid_continue))
));

named!(pub(crate) syntax<&str, &str>, map!(take_char(is_pattern_syntax), |a| a));

named!(pub(crate) whitespace<&str, &str>, take_while!(is_pattern_whitespace));

named!(pub(crate) whitespace_line<&str, &str>, take_while!(|c|
    !is_line_terminator(c) && is_pattern_whitespace(c)
));

// NOM matcher for quoted strings
// https://unicode-table.com/en/sets/quotation-marks/
// Strings are quoted with English double quotes “ ”. Quotes can be nested.

pub(crate) fn quoted(input: &str) -> IResult<&str, &str> {
    match input.chars().next() {
        None => return Err(Err::Incomplete(Needed::Size(2))),
        Some('“') => {}
        Some(_c) => return Err(Err::Error(error_position!(input, ErrorKind::Tag))), /* TODO: Custom error */
    }
    let start = '“'.len_utf8();
    let mut depth = 1;
    let mut len = 0;
    for c in input[start..].chars() {
        match c {
            '“' => depth += 1,
            '”' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        len += c.len_utf8();
    }
    if depth > 0 {
        Err(Err::Incomplete(Needed::Size(depth * '”'.len_utf8())))
    } else {
        Ok((
            &input[(start + len + '”'.len_utf8())..],
            &input[start..(start + len)],
        ))
    }
}

// NOM matcher for numbers
// TODO: Unicode numerals
// TODO: Base subscripts and exponents.
named!(pub(crate) numeral<&str, u64>,
    map_res!(digit, FromStr::from_str)
);

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
        assert_eq!(line_separator("\u{2028}.\n"), Ok((".\n", ())));
        assert_eq!(line_separator("\u{2029}.\n"), Ok((".\n", ())));
    }

    #[test]
    fn parse_identifier() {
        assert_eq!(identifier("hello"), Err(Err::Incomplete(Needed::Size(1))));
        assert_eq!(identifier("hello "), Ok((" ", "hello")));
        assert_eq!(identifier("he_llo "), Ok((" ", "he_llo")));
        assert_eq!(identifier("he-llo "), Ok(("-llo ", "he")));
        assert_eq!(identifier("he≈llo "), Ok(("≈llo ", "he")));
        assert!(identifier("_hello ").is_err());
        assert!(identifier("0123 a").is_err());
    }

    #[test]
    fn parse_whitespace() {
        assert_eq!(whitespace(""), Err(Err::Incomplete(Needed::Size(1))));
        assert_eq!(whitespace("a"), Ok(("a", "")));
        assert_eq!(whitespace(" a"), Ok(("a", " ")));
        assert_eq!(whitespace(" \t\n\r a"), Ok(("a", " \t\n\r ")));
    }

    #[test]
    fn parse_syntax() {
        assert_eq!(syntax("+ a"), Ok((" a", "+")));
        assert_eq!(syntax(". a"), Ok((" a", ".")));
        assert!(syntax("0123 a").is_err());
        assert_eq!(syntax("≈ a"), Ok((" a", "≈")));
    }

    #[test]
    fn parse_quoted() {
        assert_eq!(quoted("“Hello”asd"), Ok(("asd", "Hello")));
        assert_eq!(
            quoted("“Outer “inner” quotation” trailing input"),
            Ok((" trailing input", "Outer “inner” quotation"))
        );
        assert_eq!(quoted("“Hello””asd"), Ok(("”asd", "Hello")));
        assert_eq!(
            quoted("“1“2“3”2”“2“3““5”””2”1”0"),
            Ok(("0", "1“2“3”2”“2“3““5”””2”1"))
        );
    }

    #[test]
    fn parse_number() {
        assert_eq!(numeral("0123."), Ok((".", 123)));
    }
}
