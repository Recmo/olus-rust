#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
use crate::tokens;
use crate::Ast;
use nom::*;

pub fn is_reserved_keyword(s: &str) -> bool {
    match s {
        "“" => true,
        "”" => true,
        "↦" => true,
        "(" => true,
        ")" => true,
        _ => false,
    }
}

named!(pub identifier<&str, &str>,
    verify!(
        alt!(tokens::identifier | tokens::syntax),
        |s| !is_reserved_keyword(s)
    )
);

named!(pub binder<&str, Ast::Binder>,
    map!(identifier, |s| Ast::Binder(None, s.to_owned()))
);

named!(pub expression<&str, Ast::Expression>, alt!(
    reference | fructose | galactose | literal_string | literal_number
));

named!(pub reference<&str, Ast::Expression>,
    map!(identifier, |s| Ast::Expression::Reference(None, s.to_owned()))
);

named!(pub fructose<&str, Ast::Expression>,
    map!(
        delimited!(
            pair!(tag!("("), opt!(tokens::whitespace)),
            tuple!(
                many0!(
                    map!(pair!(binder, opt!(tokens::whitespace)), |(a, _b)| a)
                ),
                pair!(tag!("↦"), opt!(tokens::whitespace)),
                many0!(
                    map!(pair!(expression, opt!(tokens::whitespace)), |(a, _b)| a)
                )
            ),
            tag!(")")
        ),
        |(l, _m, r)| Ast::Expression::Fructose(l, r)
    )
);

named!(pub galactose<&str, Ast::Expression>,
    map!(
        delimited!(
            pair!(tag!("("), opt!(tokens::whitespace)),
            many1!(
                map!(pair!(expression, opt!(tokens::whitespace)), |(a, _b)| a)
            ),
            tag!(")")
        ),
        Ast::Expression::Galactose
    )
);

named!(pub literal_string<&str, Ast::Expression>,
    map!(
        tokens::quoted,
        |s| Ast::Expression::Literal(s.to_owned())
    )
);

named!(pub literal_number<&str, Ast::Expression>,
    map!(
        tokens::numeral,
        Ast::Expression::Number
    )
);

named!(pub closure<&str, Ast::Statement>, 
    map!(
        tuple!(
            many1!(map!(pair!(binder, opt!(tokens::whitespace_line)), |(a, _b)| a)),
            pair!(tag!("↦"), opt!(tokens::whitespace_line)),
            many0!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a))
        ),
        |(l, _m, r)| Ast::Statement::Closure(l, r)
    )
);

named!(pub call<&str, Ast::Statement>, 
    map!(
        many1!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a)),
        Ast::Statement::Call
    )
);

// Implements the off-side rule.
// TODO: Fix support for incomplete data.
named!(pub block<&str, Ast::Statement>, do_parse!(
    ident: peek!(tokens::whitespace_line) >>
    statements: many1!(
        alt_complete!(
            map!(tuple!(tag!(ident), closure, tokens::line_separator), |(_l, m, _r)| Some(m)) |
            map!(tuple!(tag!(ident), call, tokens::line_separator), |(_l, m, _r)| Some(m)) |
            map!(tuple!(
                peek!(tuple!(
                    tag!(ident),
                    verify!(tokens::whitespace_line, |s: &str| !s.is_empty())
                )),
                block
                ), |(_l, r)| Some(r)) |
            map!(tokens::line_separator, |_s| None)
        )
    ) >>
    (Ast::Statement::Block(statements.into_iter().filter_map(|v| v).collect()))
));

// Returns a single block containing the contents.
// TODO: Error handling.
pub fn parse_olus(input: &str) -> Ast::Statement {
    match block(input) {
        Ok(("", result)) => result,
        _ => panic!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn parse_galactose() {
        assert_eq!(
            expression("(\na\n\nb\n) "),
            Ok((
                " ",
                Ast::Expression::Galactose(vec![
                    Ast::Expression::Reference("a".to_string()),
                    Ast::Expression::Reference("b".to_string()),
                ])
            ))
        );
        assert_eq!(
            expression("(a_“He + (l)lo”+ (b “*”)) "),
            Ok((
                " ",
                Ast::Expression::Galactose(vec![
                    Ast::Expression::Reference("a_".to_string()),
                    Ast::Expression::Literal("He + (l)lo".to_string()),
                    Ast::Expression::Reference("+".to_string()),
                    Ast::Expression::Galactose(vec![
                        Ast::Expression::Reference("b".to_string()),
                        Ast::Expression::Literal("*".to_string()),
                    ])
                ])
            ))
        );
    }

    #[test]
    fn parse_fructose() {
        assert_eq!(
            expression("(↦)"),
            Ok(("", Ast::Expression::Fructose(vec![], vec![])))
        );
        assert_eq!(
            expression("(↦f a b)"),
            Ok((
                "",
                Ast::Expression::Fructose(
                    vec![],
                    vec![
                        Ast::Expression::Reference("f".to_string()),
                        Ast::Expression::Reference("a".to_string()),
                        Ast::Expression::Reference("b".to_string()),
                    ]
                )
            ))
        );
        assert_eq!(
            expression("(a b ↦ f)"),
            Ok((
                "",
                Ast::Expression::Fructose(
                    vec![Ast::Binder("a".to_string()), Ast::Binder("b".to_string()),],
                    vec![Ast::Expression::Reference("f".to_string()),]
                )
            ))
        );
    }

    #[test]
    fn parse_closure() {
        assert_eq!(
            closure("fact m n ↦ f a b \nc"),
            Ok((
                "\nc",
                Ast::Statement::Closure(
                    vec![
                        Ast::Binder("fact".to_string()),
                        Ast::Binder("m".to_string()),
                        Ast::Binder("n".to_string()),
                    ],
                    vec![
                        Ast::Expression::Reference("f".to_string()),
                        Ast::Expression::Reference("a".to_string()),
                        Ast::Expression::Reference("b".to_string()),
                    ]
                )
            ))
        );
    }

    #[test]
    fn parse_block() {
        fn call(a: &str) -> Ast::Statement {
            Ast::Statement::Call(vec![Ast::Expression::Reference(a.to_string())])
        }
        assert_eq!(
            block("a\nb\nc\n"),
            Ok((
                "",
                Ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block("a\nb\n\n\nc\n"),
            Ok((
                "",
                Ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block("  a\n  b\n  c\n T"),
            Ok((
                " T",
                Ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block(" a\n  b1\n\n  b2\n c\nT"),
            Ok((
                "T",
                Ast::Statement::Block(vec![
                    call("a"),
                    Ast::Statement::Block(vec![call("b1"), call("b2")]),
                    call("c")
                ])
            ))
        );
    }
}
