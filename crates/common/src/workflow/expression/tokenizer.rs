//! # Expression Tokenizer (Lexer)
//!
//! Converts an expression string into a sequence of tokens.

use std::fmt;
use thiserror::Error;

/// A token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: (usize, usize),
}

impl Token {
    pub fn new(kind: TokenKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: (start, end),
        }
    }
}

/// The kind of a token.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Integer(i64),
    Float(f64),
    StringLit(String),
    True,
    False,
    Null,

    // Identifier
    Ident(String),

    // Keywords (also parsed as identifiers initially, then classified)
    And,
    Or,
    Not,
    In,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,

    // End of input
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Integer(n) => write!(f, "{}", n),
            TokenKind::Float(n) => write!(f, "{}", n),
            TokenKind::StringLit(s) => write!(f, "\"{}\"", s),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Null => write!(f, "null"),
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::And => write!(f, "and"),
            TokenKind::Or => write!(f, "or"),
            TokenKind::Not => write!(f, "not"),
            TokenKind::In => write!(f, "in"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::BangEq => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::LtEq => write!(f, "<="),
            TokenKind::GtEq => write!(f, ">="),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Eof => write!(f, "EOF"),
        }
    }
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("Unexpected character '{0}' at position {1}")]
    UnexpectedChar(char, usize),

    #[error("Unterminated string literal starting at position {0}")]
    UnterminatedString(usize),

    #[error("Invalid number literal at position {0}: {1}")]
    InvalidNumber(usize, String),
}

/// The tokenizer / lexer.
pub struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// Tokenize the entire input and return a vector of tokens.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, TokenError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            if tok.kind == TokenKind::Eof {
                tokens.push(tok);
                break;
            }
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, TokenError> {
        self.skip_whitespace();

        let start = self.pos;

        let ch = match self.peek() {
            Some(ch) => ch,
            None => return Ok(Token::new(TokenKind::Eof, start, start)),
        };

        // Single-char and multi-char operators/delimiters
        match ch {
            '+' => {
                self.advance();
                Ok(Token::new(TokenKind::Plus, start, self.pos))
            }
            '-' => {
                self.advance();
                Ok(Token::new(TokenKind::Minus, start, self.pos))
            }
            '*' => {
                self.advance();
                Ok(Token::new(TokenKind::Star, start, self.pos))
            }
            '/' => {
                self.advance();
                Ok(Token::new(TokenKind::Slash, start, self.pos))
            }
            '%' => {
                self.advance();
                Ok(Token::new(TokenKind::Percent, start, self.pos))
            }
            '(' => {
                self.advance();
                Ok(Token::new(TokenKind::LParen, start, self.pos))
            }
            ')' => {
                self.advance();
                Ok(Token::new(TokenKind::RParen, start, self.pos))
            }
            '[' => {
                self.advance();
                Ok(Token::new(TokenKind::LBracket, start, self.pos))
            }
            ']' => {
                self.advance();
                Ok(Token::new(TokenKind::RBracket, start, self.pos))
            }
            ',' => {
                self.advance();
                Ok(Token::new(TokenKind::Comma, start, self.pos))
            }
            '.' => {
                self.advance();
                Ok(Token::new(TokenKind::Dot, start, self.pos))
            }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::new(TokenKind::EqEq, start, self.pos))
                } else {
                    Err(TokenError::UnexpectedChar('=', start))
                }
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::new(TokenKind::BangEq, start, self.pos))
                } else {
                    Err(TokenError::UnexpectedChar('!', start))
                }
            }
            '<' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::new(TokenKind::LtEq, start, self.pos))
                } else {
                    Ok(Token::new(TokenKind::Lt, start, self.pos))
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::new(TokenKind::GtEq, start, self.pos))
                } else {
                    Ok(Token::new(TokenKind::Gt, start, self.pos))
                }
            }
            '"' | '\'' => self.read_string(ch),
            c if c.is_ascii_digit() => self.read_number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.read_ident(),
            other => Err(TokenError::UnexpectedChar(other, start)),
        }
    }

    fn read_string(&mut self, quote: char) -> Result<Token, TokenError> {
        let start = self.pos;
        self.advance(); // consume opening quote
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('\\') => {
                    // Escape sequence
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        Some('\\') => s.push('\\'),
                        Some(c) if c == quote => s.push(c),
                        Some(c) => {
                            s.push('\\');
                            s.push(c);
                        }
                        None => return Err(TokenError::UnterminatedString(start)),
                    }
                }
                Some(c) if c == quote => {
                    return Ok(Token::new(TokenKind::StringLit(s), start, self.pos));
                }
                Some(c) => s.push(c),
                None => return Err(TokenError::UnterminatedString(start)),
            }
        }
    }

    fn read_number(&mut self) -> Result<Token, TokenError> {
        let start = self.pos;
        let mut num_str = String::new();
        let mut is_float = false;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else if ch == '.' && !is_float {
                // Check if this is a decimal point or a method call dot
                // Look ahead to see if next char is a digit
                let next_pos = self.pos + 1;
                if next_pos < self.chars.len() && self.chars[next_pos].is_ascii_digit() {
                    is_float = true;
                    num_str.push(ch);
                    self.advance();
                } else {
                    // It's a dot access, stop number parsing here
                    break;
                }
            } else {
                break;
            }
        }

        if is_float {
            let val: f64 = num_str
                .parse()
                .map_err(|_| TokenError::InvalidNumber(start, num_str.clone()))?;
            Ok(Token::new(TokenKind::Float(val), start, self.pos))
        } else {
            let val: i64 = num_str
                .parse()
                .map_err(|_| TokenError::InvalidNumber(start, num_str.clone()))?;
            Ok(Token::new(TokenKind::Integer(val), start, self.pos))
        }
    }

    fn read_ident(&mut self) -> Result<Token, TokenError> {
        let start = self.pos;
        let mut ident = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match ident.as_str() {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "in" => TokenKind::In,
            _ => TokenKind::Ident(ident),
        };

        Ok(Token::new(kind, start, self.pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<TokenKind> {
        let mut t = Tokenizer::new(input);
        t.tokenize().unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn test_simple_expression() {
        let kinds = tokenize("2 + 3");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(2),
                TokenKind::Plus,
                TokenKind::Integer(3),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_comparison() {
        let kinds = tokenize("x >= 10");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("x".to_string()),
                TokenKind::GtEq,
                TokenKind::Integer(10),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_keywords() {
        let kinds = tokenize("true and not false or null in x");
        assert_eq!(
            kinds,
            vec![
                TokenKind::True,
                TokenKind::And,
                TokenKind::Not,
                TokenKind::False,
                TokenKind::Or,
                TokenKind::Null,
                TokenKind::In,
                TokenKind::Ident("x".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_string_literals() {
        let kinds = tokenize("\"hello\" + 'world'");
        assert_eq!(
            kinds,
            vec![
                TokenKind::StringLit("hello".to_string()),
                TokenKind::Plus,
                TokenKind::StringLit("world".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_float() {
        let kinds = tokenize("3.14");
        assert_eq!(kinds, vec![TokenKind::Float(3.14), TokenKind::Eof]);
    }

    #[test]
    fn test_dot_access() {
        let kinds = tokenize("obj.field");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("obj".to_string()),
                TokenKind::Dot,
                TokenKind::Ident("field".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_function_call() {
        let kinds = tokenize("length(arr)");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("length".to_string()),
                TokenKind::LParen,
                TokenKind::Ident("arr".to_string()),
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_bracket_access() {
        let kinds = tokenize("arr[0]");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("arr".to_string()),
                TokenKind::LBracket,
                TokenKind::Integer(0),
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_escape_sequences() {
        let kinds = tokenize(r#""hello\nworld""#);
        assert_eq!(
            kinds,
            vec![
                TokenKind::StringLit("hello\nworld".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_integer_followed_by_dot() {
        // `42.field` - the 42 is an integer and `.field` is separate
        let kinds = tokenize("42.field");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Integer(42),
                TokenKind::Dot,
                TokenKind::Ident("field".to_string()),
                TokenKind::Eof,
            ]
        );
    }
}
