#![allow(clippy::use_self)] // False positive from macro
#![allow(clippy::non_ascii_literal)] // Syntax is non-ascii

use logos::Logos;
use std::cmp::Ordering;

pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'source> {
    BlockStart,
    BlockEnd,
    LineStart,
    LineEnd,
    Identifier(&'source str),
    String(&'source str),
    Error(Error, Span),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    TokenError,
    IndentationError,
    StringError,
    StringUnterminated,
}

pub struct Lexer<'source> {
    lexer:        logos::Lexer<'source, RawToken>,
    next_token:   Option<RawToken>,
    line_started: bool,
    indent_stack: Vec<usize>,
    next_indent:  usize,
}

#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(subpattern newline=r"[\u000a\u000b\u000c\u000d\u0085\u2028\u2029]")]
enum RawToken {
    // White space excluding line breaks
    // See <https://www.unicode.org/reports/tr14>
    // See <https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=[:Pattern_White_Space=Yes:]>
    #[regex(r"[\p{Pattern_White_Space}--(?&newline)]+")]
    Whitespace,

    // Line breaks
    #[regex(r"[\p{Pattern_White_Space}&&(?&newline)]+")]
    Newline,

    // Identifiers and symbols
    // See <https://www.unicode.org/reports/tr31>
    // See <https://util.unicode.org/UnicodeJsps/list-unicodeset.jsp?a=[:Pattern_Syntax=Yes:]>
    #[regex(r"\p{XID_Start}\p{XID_Continue}*|\p{Pattern_Syntax}")]
    Identifier,

    #[token("“")]
    StringStart,

    #[error]
    Error,
}

#[derive(Logos, Debug, Clone, Copy, PartialEq)]
enum LiteralString {
    #[token("“")]
    StringStart,

    #[token("”")]
    StringStop,

    #[regex(r"[^“”]+")]
    Characters,

    #[error]
    Error,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Lexer {
            lexer:        RawToken::lexer(source),
            line_started: false,
            indent_stack: vec![],
            next_indent:  0,
            next_token:   None,
        }
    }

    pub fn source(&self) -> &'source str {
        self.lexer.source()
    }

    const fn indentation_length(str: &str) -> usize {
        // Indentation length currently equals number of characters
        str.len()
    }

    fn parse_string(&mut self) -> Token<'source> {
        let mut lexer: logos::Lexer<_> = LiteralString::lexer(self.lexer.remainder());
        let mut nesting = 0_usize;
        loop {
            match lexer.next() {
                Some(LiteralString::StringStart) => nesting += 1,
                Some(LiteralString::StringStop) => {
                    if let Some(value) = nesting.checked_sub(1) {
                        nesting = value
                    } else {
                        let result = &self.lexer.remainder()[0..lexer.span().start];
                        self.lexer.bump(lexer.span().end);
                        break Token::String(result);
                    }
                }
                Some(LiteralString::Characters) => {}
                Some(LiteralString::Error) => break Token::Error(Error::StringError, lexer.span()),
                None => break Token::Error(Error::StringUnterminated, 0..lexer.span().end),
            }
        }
    }
}

impl<'source> Iterator for Lexer<'source> {
    type Item = Token<'source>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_token.is_none() {
            self.next_token = self.lexer.next();
        }
        match self.next_token {
            None => None,
            Some(RawToken::Newline) => {
                self.next_indent = 0;
                self.next_token = None;
                if self.line_started {
                    self.line_started = false;
                    Some(Token::LineEnd)
                } else {
                    self.next()
                }
            }
            Some(RawToken::Whitespace) => {
                if !self.line_started {
                    // TODO: Compute indent_size
                    self.next_indent = Self::indentation_length(self.lexer.slice());
                }
                self.next_token = None;
                self.next()
            }
            Some(token) => {
                if self.line_started {
                    self.next_token = None;
                    match token {
                        RawToken::Identifier => Some(Token::Identifier(self.lexer.slice())),
                        RawToken::Error => Some(Token::Error(Error::TokenError, self.lexer.span())),
                        RawToken::StringStart => Some(self.parse_string()),
                        _ => unreachable!(),
                    }
                } else {
                    let mut last_indent = self.indent_stack.last().copied().unwrap_or_default();
                    match self.next_indent.cmp(&last_indent) {
                        Ordering::Greater => {
                            self.indent_stack.push(self.next_indent);
                            Some(Token::BlockStart)
                        }
                        Ordering::Less => {
                            self.indent_stack.pop();
                            last_indent = self.indent_stack.last().copied().unwrap_or_default();
                            if self.next_indent > last_indent {
                                // We un-indented back to a level not seen before,
                                // this is an error.
                                // TODO: Ideally we recover with [Error, BlockEnd, BlockStart]
                                // for consistency.
                                Some(Token::Error(Error::IndentationError, self.lexer.span()))
                            } else {
                                Some(Token::BlockEnd)
                            }
                        }
                        Ordering::Equal => {
                            self.line_started = true;
                            Some(Token::LineStart)
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use logos::Span;

    fn parse<'a, T>(source: &'a str) -> Vec<(T, Span)>
    where
        T: Logos<'a, Source = str, Extras = ()>,
    {
        let lexer = T::lexer(source);
        lexer.spanned().collect()
    }

    #[test]
    fn test_whitespace() {
        use RawToken::*;
        assert_eq!(parse("\n"), vec![(Newline, 0..1)]);
        assert_eq!(parse("\r"), vec![(Newline, 0..1)]);
        assert_eq!(parse("\u{2028}"), vec![(Newline, 0..3)]);
        assert_eq!(parse("\u{2029}"), vec![(Newline, 0..3)]);
        assert_eq!(parse("\t"), vec![(Whitespace, 0..1)]);
        assert_eq!(parse(" "), vec![(Whitespace, 0..1)]);
    }

    #[test]
    fn test_identifier() {
        use RawToken::*;
        assert_eq!(parse("hello"), vec![(Identifier, 0..5)]);
        assert_eq!(parse("hello "), vec![
            (Identifier, 0..5),
            (Whitespace, 5..6)
        ]);
        assert_eq!(parse("he_llo "), vec![
            (Identifier, 0..6),
            (Whitespace, 6..7)
        ]);
        assert_eq!(parse("he-llo "), vec![
            (Identifier, 0..2),
            (Identifier, 2..3),
            (Identifier, 3..6),
            (Whitespace, 6..7)
        ]);
        assert_eq!(parse("he≈llo "), vec![
            (Identifier, 0..2),
            (Identifier, 2..5),
            (Identifier, 5..8),
            (Whitespace, 8..9)
        ]);
        assert_eq!(parse("_hello"), vec![(Error, 0..1), (Identifier, 1..6)]);
        // assert_eq!(parse("0123 a"), vec![(Identifier, 0..5)]);
        assert_eq!(parse("+-asd"), vec![
            (Identifier, 0..1),
            (Identifier, 1..2),
            (Identifier, 2..5)
        ]);
    }

    #[test]
    #[rustfmt::skip]
    fn test_block() {
        use Token::*;
        assert_eq!(
            Lexer::new(indoc!(r#"
                def hello
                    setup
                    for foo
                        if foo
                            print foo

                    aap

                hello
            "#)).collect::<Vec<_>>(),
            vec![
                LineStart, Identifier("def"), Identifier("hello"), LineEnd,
                BlockStart,
                    LineStart, Identifier("setup"), LineEnd,
                    LineStart, Identifier("for"), Identifier("foo"), LineEnd,
                    BlockStart,
                        LineStart, Identifier("if"), Identifier("foo"), LineEnd,
                        BlockStart,
                            LineStart, Identifier("print"), Identifier("foo"), LineEnd,
                        BlockEnd,
                    BlockEnd,
                    LineStart, Identifier("aap"), LineEnd,
                BlockEnd,
                LineStart, Identifier("hello"), LineEnd,
            ]
        );
    }

    #[test]
    fn test_string() {
        use Token::*;
        assert_eq!(Lexer::new("“Hello”asd").collect::<Vec<_>>(), vec![
            LineStart,
            String("Hello"),
            Identifier("asd")
        ]);
        assert_eq!(
            Lexer::new("“Outer “inner” quotation” trailing input").collect::<Vec<_>>(),
            vec![
                LineStart,
                String("Outer “inner” quotation"),
                Identifier("trailing"),
                Identifier("input")
            ]
        );
        assert_eq!(Lexer::new("“Hello””asd").collect::<Vec<_>>(), vec![
            LineStart,
            String("Hello"),
            Identifier("”"),
            Identifier("asd")
        ]);
        assert_eq!(
            Lexer::new("“1“2“3”2”“2“3““5”””2”1”a").collect::<Vec<_>>(),
            vec![LineStart, String("1“2“3”2”“2“3““5”””2”1"), Identifier("a")]
        );
    }
}
