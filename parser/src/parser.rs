#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
use crate::{ast, tokens};
use nom::*;

pub(crate) fn is_reserved_keyword(s: &str) -> bool {
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

named!(pub binder<&str, ast::Binder>,
    map!(identifier, |s| ast::Binder(None, s.to_owned()))
);

named!(pub expression<&str, ast::Expression>, alt!(
    reference | fructose | galactose | literal_string | literal_number
));

named!(pub reference<&str, ast::Expression>,
    map!(identifier, |s| ast::Expression::Reference(None, s.to_owned()))
);

named!(pub fructose<&str, ast::Expression>,
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
        |(l, _m, r)| ast::Expression::Fructose(l, r)
    )
);

named!(pub galactose<&str, ast::Expression>,
    map!(
        delimited!(
            pair!(tag!("("), opt!(tokens::whitespace)),
            many1!(
                map!(pair!(expression, opt!(tokens::whitespace)), |(a, _b)| a)
            ),
            tag!(")")
        ),
        ast::Expression::Galactose
    )
);

named!(pub literal_string<&str, ast::Expression>,
    map!(
        tokens::quoted,
        |s| ast::Expression::Literal(s.to_owned())
    )
);

named!(pub literal_number<&str, ast::Expression>,
    map!(
        tokens::numeral,
        ast::Expression::Number
    )
);

named!(pub closure<&str, ast::Statement>, 
    map!(
        tuple!(
            many1!(map!(pair!(binder, opt!(tokens::whitespace_line)), |(a, _b)| a)),
            pair!(tag!("↦"), opt!(tokens::whitespace_line)),
            many0!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a))
        ),
        |(l, _m, r)| ast::Statement::Closure(l, r)
    )
);

named!(pub call<&str, ast::Statement>, 
    map!(
        many1!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a)),
        ast::Statement::Call
    )
);

// Implements the off-side rule.
// TODO: Fix support for incomplete data.
named!(pub block<&str, ast::Statement>, do_parse!(
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
    (ast::Statement::Block(statements.into_iter().filter_map(|v| v).collect()))
));

// Returns a single block containing the contents.
// TODO: Error handling.
pub(crate) fn parse_olus(input: &str) -> ast::Statement {
    match block(input) {
        Ok(("", result)) => result,
        _ => panic!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_galactose() {
        assert_eq!(
            expression("(\na\n\nb\n) "),
            Ok((
                " ",
                ast::Expression::Galactose(vec![
                    ast::Expression::Reference(None, "a".to_string()),
                    ast::Expression::Reference(None, "b".to_string()),
                ])
            ))
        );
        assert_eq!(
            expression("(a_“He + (l)lo”+ (b “*”)) "),
            Ok((
                " ",
                ast::Expression::Galactose(vec![
                    ast::Expression::Reference(None, "a_".to_string()),
                    ast::Expression::Literal("He + (l)lo".to_string()),
                    ast::Expression::Reference(None, "+".to_string()),
                    ast::Expression::Galactose(vec![
                        ast::Expression::Reference(None, "b".to_string()),
                        ast::Expression::Literal("*".to_string()),
                    ])
                ])
            ))
        );
    }

    #[test]
    fn parse_fructose() {
        assert_eq!(
            expression("(↦)"),
            Ok(("", ast::Expression::Fructose(vec![], vec![])))
        );
        assert_eq!(
            expression("(↦f a b)"),
            Ok((
                "",
                ast::Expression::Fructose(vec![], vec![
                    ast::Expression::Reference(None, "f".to_string()),
                    ast::Expression::Reference(None, "a".to_string()),
                    ast::Expression::Reference(None, "b".to_string()),
                ])
            ))
        );
        assert_eq!(
            expression("(a b ↦ f)"),
            Ok((
                "",
                ast::Expression::Fructose(
                    vec![
                        ast::Binder(None, "a".to_string()),
                        ast::Binder(None, "b".to_string()),
                    ],
                    vec![ast::Expression::Reference(None, "f".to_string()),]
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
                ast::Statement::Closure(
                    vec![
                        ast::Binder(None, "fact".to_string()),
                        ast::Binder(None, "m".to_string()),
                        ast::Binder(None, "n".to_string()),
                    ],
                    vec![
                        ast::Expression::Reference(None, "f".to_string()),
                        ast::Expression::Reference(None, "a".to_string()),
                        ast::Expression::Reference(None, "b".to_string()),
                    ]
                )
            ))
        );
    }

    #[test]
    fn parse_block() {
        fn call(a: &str) -> ast::Statement {
            ast::Statement::Call(vec![ast::Expression::Reference(None, a.to_string())])
        }
        assert_eq!(
            block("a\nb\nc\n"),
            Ok((
                "",
                ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block("a\nb\n\n\nc\n"),
            Ok((
                "",
                ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block("  a\n  b\n  c\n T"),
            Ok((
                " T",
                ast::Statement::Block(vec![call("a"), call("b"), call("c")])
            ))
        );
        assert_eq!(
            block(" a\n  b1\n\n  b2\n c\nT"),
            Ok((
                "T",
                ast::Statement::Block(vec![
                    call("a"),
                    ast::Statement::Block(vec![call("b1"), call("b2")]),
                    call("c")
                ])
            ))
        );
    }
}
