use crate::{
    ast::{Binder, Expression, Statement},
    lexer::{Error, Lexer, Span, Token},
};

struct Parser<'source> {
    lexer: Lexer<'source>,
}

impl<'source> Parser<'source> {
    pub fn new(source: &'source str) -> Self {
        Parser {
            lexer: Lexer::new(source),
        }
    }

    pub fn parse(&mut self) {
        while let Some(token) = self.lexer.next() {
            match token {
                Token::BlockStart => {
                    dbg!(self.parse_block());
                }
                Token::LineStart => {
                    dbg!(self.parse_line());
                }
                _ => {
                    println!("Invalid token {:?}", token);
                }
            }
        }
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
                _ => {
                    println!("Unexpected line token {:?}", token);
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

fn parse(source: &str) -> Statement {
    let mut parser = Parser::new(source);
    parser.parse();

    Statement::Block(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let source = include_str!("../../simple.olus");
        parse(source);
    }
}
