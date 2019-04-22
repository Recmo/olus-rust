#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
use crate::AST;
use crate::tokens;
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

named!(pub binder<&str, AST::Binder>,
    map!(identifier, |s| AST::Binder(None, s.to_owned()))
);

named!(pub expression<&str, AST::Expression>, alt!(
    reference | fructose | galactose | literal_string | literal_number
));

named!(pub reference<&str, AST::Expression>,
    map!(identifier, |s| AST::Expression::Reference(None, s.to_owned()))
);

named!(pub fructose<&str, AST::Expression>,
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
        |(l, _m, r)| AST::Expression::Fructose(l, r)
    )
);

named!(pub galactose<&str, AST::Expression>,
    map!(
        delimited!(
            pair!(tag!("("), opt!(tokens::whitespace)),
            many1!(
                map!(pair!(expression, opt!(tokens::whitespace)), |(a, _b)| a)
            ),
            tag!(")")
        ),
        AST::Expression::Galactose
    )
);

named!(pub literal_string<&str, AST::Expression>,
    map!(
        tokens::quoted,
        |s| AST::Expression::Literal(s.to_owned())
    )
);

named!(pub literal_number<&str, AST::Expression>,
    map!(
        tokens::numeral,
        AST::Expression::Number
    )
);

named!(pub closure<&str, AST::Statement>, 
    map!(
        tuple!(
            many1!(map!(pair!(binder, opt!(tokens::whitespace_line)), |(a, _b)| a)),
            pair!(tag!("↦"), opt!(tokens::whitespace_line)),
            many0!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a))
        ),
        |(l, _m, r)| AST::Statement::Closure(l, r)
    )
);

named!(pub call<&str, AST::Statement>, 
    map!(
        many1!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a)),
        AST::Statement::Call
    )
);

// Implements the off-side rule.
// TODO: Fix support for incomplete data.
named!(pub block<&str, AST::Statement>, do_parse!(
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
    (AST::Statement::Block(statements.into_iter().filter_map(|v| v).collect()))
));

// Returns a single block containing the contents.
// TODO: Error handling.
pub fn parse_olus(input: &str) -> AST::Statement {
    match block(input) {
        Ok(("", result)) => result,
        _ => panic!()
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
                AST::Expression::Galactose(vec![
                    AST::Expression::Reference("a".to_string()),
                    AST::Expression::Reference("b".to_string()),
                ])
            ))
        );
        assert_eq!(
            expression("(a_“He + (l)lo”+ (b “*”)) "),
            Ok((
                " ",
                AST::Expression::Galactose(vec![
                    AST::Expression::Reference("a_".to_string()),
                    AST::Expression::Literal("He + (l)lo".to_string()),
                    AST::Expression::Reference("+".to_string()),
                    AST::Expression::Galactose(vec![
                        AST::Expression::Reference("b".to_string()),
                        AST::Expression::Literal("*".to_string()),
                    ])
                ])
            ))
        );
    }

    #[test]
    fn parse_fructose() {
        assert_eq!(
            expression("(↦)"),
            Ok(("", AST::Expression::Fructose(vec![], vec![])))
        );
        assert_eq!(
            expression("(↦f a b)"),
            Ok((
                "",
                AST::Expression::Fructose(
                    vec![],
                    vec![
                        AST::Expression::Reference("f".to_string()),
                        AST::Expression::Reference("a".to_string()),
                        AST::Expression::Reference("b".to_string()),
                    ]
                )
            ))
        );
        assert_eq!(
            expression("(a b ↦ f)"),
            Ok((
                "",
                AST::Expression::Fructose(
                    vec![AST::Binder("a".to_string()), AST::Binder("b".to_string()),],
                    vec![AST::Expression::Reference("f".to_string()),]
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
                AST::Statement::Closure(
                    vec![
                        AST::Binder("fact".to_string()),
                        AST::Binder("m".to_string()),
                        AST::Binder("n".to_string()),
                    ],
                    vec![
                        AST::Expression::Reference("f".to_string()),
                        AST::Expression::Reference("a".to_string()),
                        AST::Expression::Reference("b".to_string()),
                    ]
                )
            ))
        );
    }

    #[test]
    fn parse_block() {
        fn call(a: &str) -> AST::Statement {
            AST::Statement::Call(vec![AST::Expression::Reference(a.to_string())])
        }
        assert_eq!(block("a\nb\nc\n"), Ok((
            "", 
            AST::Statement::Block(vec![call("a"), call("b"), call("c")])
        )));
        assert_eq!(block("a\nb\n\n\nc\n"), Ok((
            "", 
            AST::Statement::Block(vec![call("a"), call("b"), call("c")])
        )));
        assert_eq!(block("  a\n  b\n  c\n T"), Ok((
            " T", 
            AST::Statement::Block(vec![call("a"), call("b"), call("c")])
        )));
        assert_eq!(block(" a\n  b1\n\n  b2\n c\nT"), Ok((
            "T", 
            AST::Statement::Block(vec![
                call("a"), 
                AST::Statement::Block(vec![
                    call("b1"),
                    call("b2")
                ])
                , call("c")
            ])
        )));
    }
}
