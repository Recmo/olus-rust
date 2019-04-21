#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
mod tokens;

use nom::*;
use tokens::{identifier, syntax, whitespace_line};
pub use unic::UNICODE_VERSION;

named!(name<&str, &str>, alt!(identifier | verify!(syntax, |s| match s {
    "↦" => false,
    "(" => false,
    ")" => false,
    "“" => false,
    "”" => false,
    _ => true,
})));

named!(line<&str, Vec<&str> >, separated_nonempty_list!(opt!(whitespace_line), name));

named!(maplet<&str, ()>, value!((), char!('↦')));

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn parse_name() {
        assert_eq!(name("test asd"), Ok((" asd", "test")));
        assert_eq!(name("+asd sdf"), Ok(("asd sdf", "+")));
        assert_eq!(name("+*asd sdf"), Ok(("*asd sdf", "+")));
        assert!(name("(*asd sdf").is_err());
    }

    #[test]
    fn parse_maplet() {
        assert_eq!(maplet("↦test"), Ok(("test", ())));
        assert!(maplet("+asd sdf").is_err());
    }

    #[test]
    fn parse_line() {
        assert!(line(".").is_err());
        assert!(line("").is_err());
        assert_eq!(
            line("fact n\t m a."),
            Ok((".", vec!["fact", "n", "m", "a"]))
        );
        assert_eq!(line("fact n\n m a."), Ok(("\n m a.", vec!["fact", "n"])));
        assert_eq!(
            line("a + b * c\n"),
            Ok(("\n", vec!["a", "+", "b", "*", "c"]))
        );

        // TODO: Allow spliting on syntax. While we don't support infix notation
        // it still makes sense as there is no other valid parse.
        assert_eq!(line("a+b*c\n"), Ok(("\n", vec!["a", "+", "b", "*", "c"])));
        assert_eq!(line("+*/\n"), Ok(("\n", vec!["+", "*", "/"])));
    }
}
