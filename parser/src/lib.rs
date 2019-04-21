use nom::*;
pub use unic::normal::StrNormalForm;
use unic::ucd::ident::{is_pattern_syntax, is_pattern_whitespace, is_xid_continue, is_xid_start};
pub use unic::UNICODE_VERSION;

//
// NOM matchers for unicode UAX31
//

// @see https://www.unicode.org/reports/tr31/

named!(identifier<&str, String>, do_parse!(
    head: take_while_m_n!(1, 1, is_xid_start) >>
    tail: take_while!(is_xid_continue) >>
    (head.to_owned() + tail)
));

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
    fn parse_identifier() {
        println!("{:?}", identifier("hello"));
        assert_eq!(identifier("hello"), Err(Err::Incomplete(Needed::Size(1))));
        assert_eq!(identifier("hello "), Ok((" ", "hello".to_owned())));
    }
}
