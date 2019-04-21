use nom::types::CompleteStr;
use nom::*;
use nom_locate::{position, LocatedSpan};
pub use unic::normal::StrNormalForm;
use unic::ucd::ident::{is_pattern_syntax, is_pattern_whitespace, is_xid_continue, is_xid_start};
pub use unic::UNICODE_VERSION;

type Span<'a> = LocatedSpan<CompleteStr<'a>>;

//
// NOM matchers for unicode UAX31
//

// @see https://www.unicode.org/reports/tr31/

#[derive(Debug)]
struct Identifier<'a>(String, Span<'a>);
// TODO: NFKC. Case invariant equality. Maybe case folding?

impl<'a> Identifier<'a> {
    pub fn new(id: &str, location: Span<'a>) -> Identifier<'a> {
        // TODO: Implement https://www.unicode.org/reports/tr31/#NFKC_Modifications
        Identifier(id.nfkc().collect(), location)
    }

    pub fn matches_case() -> bool {
        // Make sure that the case is identical
        false
    }
}

impl<'a> PartialEq for Identifier<'a> {
    fn eq(&self, other: &Identifier) -> bool {
        false
    }
}

#[derive(Debug)]
struct Syntax<'a>(String, Span<'a>);

#[allow(clippy::double_comparisons)]
named!(identifier(Span) -> Identifier, do_parse!(
    position: position!() >>
    head: take_while_m_n!(1, 1, is_xid_start) >>
    tail: take_while!(is_xid_continue) >>
    (Identifier::new(&(head.to_string() + tail.fragment.0), position))
));

named!(whitespace(Span) -> (), do_parse!(
    take_while!(is_pattern_whitespace) >>
    ()
));

named!(syntax(Span) -> Syntax, do_parse!(
    position: position!() >>
    operator: take_while!(is_pattern_syntax) >>
    (Syntax(operator.to_string(), position))
));

//
// NOM matchers for strings and numbers
//

// https://unicode-table.com/en/sets/quotation-marks/
// Strings are quoted with English double quotes “ ”. Quotes can be nested.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_identifier() {
        println!("{:?}", identifier("hello"));
        //assert_eq!(Ok(Identifier("hello")), identifier("hello"))
    }
}
