#![deny(clippy::all)]
#![allow(clippy::double_comparisons)] // Many false positives with nom macros.
mod AST;
mod tokens;

use nom::*;
pub use unic::UNICODE_VERSION;

fn is_reserved_keyword(s: &str) -> bool {
    match s {
        "“" => true,
        "”" => true,
        "↦" => true,
        "(" => true,
        ")" => true,
        _ => false,
    }
}

named!(identifier<&str, &str>,
    verify!(
        alt!(tokens::identifier | tokens::syntax),
        |s| !is_reserved_keyword(s)
    )
);

named!(binder<&str, AST::Binder>,
    map!(identifier, |s| AST::Binder(s.to_owned()))
);

named!(expression<&str, AST::Expression>, alt!(
    reference | fructose | galactose | literal_string
));

named!(reference<&str, AST::Expression>,
    map!(identifier, |s| AST::Expression::Reference(s.to_owned()))
);

named!(fructose<&str, AST::Expression>,
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

named!(galactose<&str, AST::Expression>,
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

named!(literal_string<&str, AST::Expression>,
    map!(
        tokens::quoted,
        |s| AST::Expression::Literal(s.to_owned())
    )
);

named!(statement<&str, AST::Statement>, alt!(
    closure | call | block
));

named!(closure<&str, AST::Statement>, 
    map!(
        tuple!(
            many1!(map!(pair!(binder, opt!(tokens::whitespace_line)), |(a, _b)| a)),
            pair!(tag!("↦"), opt!(tokens::whitespace_line)),
            many0!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a))
        ),
        |(l, _m, r)| AST::Statement::Closure(l, r)
    )
);

named!(call<&str, AST::Statement>, 
    map!(
        many1!(map!(pair!(expression, opt!(tokens::whitespace_line)), |(a, _b)| a)),
        AST::Statement::Call
    )
);

// Implements the off-side rule.
named!(block<&str, AST::Statement>, do_parse!(
    ident: peek!(verify!(tokens::whitespace_line, |s: &str| s.len() > 0)) >>
    statements: many1!(
        // TODO: Nested block needs to have ident concatenated or it will have a false value
        // for the next line.
        map!(tuple!(tag!(ident), statement, tokens::line_separator), |(_l, m, _r)| m)
    ) >>
    (AST::Statement::Block(statements))
));

named!(line<&str, Vec<&str> >, many1!(
    map!(pair!(alt!(tokens::identifier | tokens::syntax), opt!(tokens::whitespace_line)), |(a, _b)| a)
));

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
        println!("{:?}", block("  a\n  b\n  c\n T"));
        println!("{:?}", block(" a\n  b1\n  b2\n c\nT"));
    }

    #[test]
    fn parse_line() {
        println!("{:?}", line(" fact\n"));
        assert!(line(".").is_err());
        assert!(line("").is_err());
        assert!(line(" fact\n").is_err());
        assert_eq!(
            line("fact n\t m a.\n"),
            Ok(("\n", vec!["fact", "n", "m", "a", "."]))
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
