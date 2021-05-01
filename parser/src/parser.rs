use crate::{
    ast::{Binder, Expression, Statement},
    lexer::{Error, Lexer, Span, Token},
};

pub struct Parser<'source> {
    lexer: Lexer<'source>,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str) -> Self {
        Parser {
            lexer: Lexer::new(source),
        }
    }

    pub fn parse(&mut self) -> Statement {
        self.parse_block()
    }

    fn print_diagnostic(&self, error: Error, span: Span) {
        use codespan_reporting::{
            diagnostic::{Diagnostic, Label},
            files::SimpleFile,
            term::{
                self,
                termcolor::{ColorChoice, StandardStream},
            },
        };

        let file = SimpleFile::new("source", self.lexer.source());
        let diagnostic = Diagnostic::error()
            .with_message(format!("Error {:?}", error))
            .with_labels(vec![Label::primary((), span)]);

        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();
        term::emit(&mut writer.lock(), &config, &file, &diagnostic).unwrap();
    }

    fn parse_block(&mut self) -> Statement {
        let mut statements = vec![];
        while let Some(token) = self.lexer.next() {
            match token {
                Token::BlockStart => {
                    statements.push(self.parse_block());
                }
                Token::LineStart => {
                    statements.push(self.parse_line());
                }
                Token::BlockEnd => break,
                _ => {
                    println!("Unexpected block token {:?}", token);
                }
            }
        }
        Statement::Block(statements)
    }

    fn parse_line(&mut self) -> Statement {
        let mut line = vec![];
        let mut maplet_pos = None;
        while let Some(token) = self.lexer.next() {
            match token {
                Token::Identifier("↦") => {
                    if maplet_pos.is_some() {
                        println!("Maplet already found.");
                    } else {
                        maplet_pos = Some(line.len());
                    }
                }
                Token::Identifier("(") => line.push(self.parse_paren()),
                Token::Identifier(name) => {
                    line.push(Expression::Reference(None, name.to_owned()));
                }
                Token::String(str) => {
                    line.push(Expression::Literal(str.to_owned()));
                }
                Token::Number(n) => {
                    line.push(Expression::Number(n));
                }
                Token::LineEnd => break,
                Token::Error(error, span) => self.print_diagnostic(error, span),
                _ => {
                    println!("Unexpected line token {:?}", token);
                }
            }
        }
        if let Some(maplet_pos) = maplet_pos {
            let (left, right) = line.split_at(maplet_pos);
            assert!(!left.is_empty());
            let mut binders = Vec::with_capacity(left.len());
            for exp in left {
                match exp {
                    Expression::Reference(_, name) => {
                        binders.push(Binder(None, name.to_string()));
                    }
                    _ => {
                        println!("Expected binder");
                    }
                }
            }
            Statement::Closure(binders, right.to_vec())
        } else {
            Statement::Call(line)
        }
    }

    fn parse_paren(&mut self) -> Expression {
        let mut line = vec![];
        let mut maplet_pos = None;
        while let Some(token) = self.lexer.next() {
            match token {
                Token::Identifier("↦") => {
                    if maplet_pos.is_some() {
                        println!("Maplet already found.");
                    } else {
                        maplet_pos = Some(line.len());
                    }
                }
                Token::Identifier("(") => line.push(self.parse_paren()),
                Token::Identifier(")") => break,
                Token::Identifier(name) => {
                    line.push(Expression::Reference(None, name.to_owned()));
                }
                Token::String(str) => {
                    line.push(Expression::Literal(str.to_owned()));
                }
                Token::Number(n) => {
                    line.push(Expression::Number(n));
                }
                Token::BlockStart | Token::BlockEnd | Token::LineStart | Token::LineEnd => {
                    // Ignore lines.
                    // TODO: Make sure they don't confuse indentation state
                }
                _ => {
                    println!("Unexpected paren token {:?}", token);
                }
            }
        }
        if let Some(maplet_pos) = maplet_pos {
            let (left, right) = line.split_at(maplet_pos);
            let mut binders = Vec::with_capacity(left.len());
            for exp in left {
                match exp {
                    Expression::Reference(_, name) => {
                        binders.push(Binder(None, name.to_string()));
                    }
                    _ => {
                        println!("Expected binder");
                    }
                }
            }
            Expression::Fructose(binders, right.to_vec())
        } else {
            Expression::Galactose(line)
        }
    }
}

pub fn parse(source: &str) -> Statement {
    let mut parser = Parser::new(source);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn wrap_expr(expr: Expression) -> Statement {
        Statement::Block(vec![Statement::Call(vec![expr])])
    }

    #[test]
    fn parse_galactose() {
        assert_eq!(
            parse("(\na\n\nb\n) "),
            wrap_expr(Expression::Galactose(vec![
                Expression::Reference(None, "a".to_string()),
                Expression::Reference(None, "b".to_string()),
            ]))
        );
        assert_eq!(
            parse("(a_“He + (l)lo”+ (b “*”)) "),
            wrap_expr(Expression::Galactose(vec![
                Expression::Reference(None, "a_".to_string()),
                Expression::Literal("He + (l)lo".to_string()),
                Expression::Reference(None, "+".to_string()),
                Expression::Galactose(vec![
                    Expression::Reference(None, "b".to_string()),
                    Expression::Literal("*".to_string()),
                ])
            ]))
        );
    }

    #[test]
    fn parse_fructose() {
        assert_eq!(
            parse("(↦)"),
            wrap_expr(Expression::Fructose(vec![], vec![]))
        );
        assert_eq!(
            parse("(↦f a b)"),
            wrap_expr(Expression::Fructose(vec![], vec![
                Expression::Reference(None, "f".to_string()),
                Expression::Reference(None, "a".to_string()),
                Expression::Reference(None, "b".to_string()),
            ]))
        );
        assert_eq!(
            parse("(a b ↦ f)"),
            wrap_expr(Expression::Fructose(
                vec![Binder(None, "a".to_string()), Binder(None, "b".to_string()),],
                vec![Expression::Reference(None, "f".to_string()),]
            ))
        );
    }

    #[test]
    fn parse_closure() {
        assert_eq!(
            parse("fact m n ↦ f a b \nc"),
            Statement::Block(vec![Statement::Closure(
                vec![
                    Binder(None, "fact".to_string()),
                    Binder(None, "m".to_string()),
                    Binder(None, "n".to_string()),
                ],
                vec![
                    Expression::Reference(None, "f".to_string()),
                    Expression::Reference(None, "a".to_string()),
                    Expression::Reference(None, "b".to_string()),
                ]
            ),
            Statement::Call(vec![Expression::Reference(None, "c".to_string())])
            ])
        );
    }

    // #[test]
    // fn parse_block() {
    //     fn call(a: &str) -> Statement {
    //         Statement::Call(vec![Expression::Reference(None, a.to_string())])
    //     }
    //     assert_eq!(
    //         parse("a\nb\nc\n"),
    //         Statement::Block(vec![call("a"), call("b"), call("c")])
    //     );
    //     assert_eq!(
    //         parse("a\nb\n\n\nc\n"),
    //         Statement::Block(vec![call("a"), call("b"), call("c")])
    //     );
    //     assert_eq!(
    //         parse("  a\n  b\n  c\n T"),
    //         Statement::Block(vec![call("a"), call("b"), call("c")])
    //     );
    //     assert_eq!(
    //         parse(" a\n  b1\n\n  b2\n c\nT"),
    //         Statement::Block(vec![
    //             call("a"),
    //             Statement::Block(vec![call("b1"), call("b2")]),
    //             call("c")
    //         ])
    //     );
    // }
}
