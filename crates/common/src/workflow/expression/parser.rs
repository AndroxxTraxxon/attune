//! # Expression Parser
//!
//! Recursive-descent parser that transforms a token stream into an AST.
//!
//! ## Operator Precedence (lowest to highest)
//!
//! 1. `or`
//! 2. `and`
//! 3. `not` (unary)
//! 4. `==`, `!=`, `<`, `>`, `<=`, `>=`, `in`
//! 5. `+`, `-` (addition / subtraction)
//! 6. `*`, `/`, `%`
//! 7. Unary `-`
//! 8. Postfix: `.field`, `[index]`, `(args)`

use super::ast::{BinaryOp, Expr, UnaryOp};
use super::tokenizer::{Token, TokenKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unexpected token {0} at position {1}")]
    UnexpectedToken(String, usize),

    #[error("Expected {0}, found {1} at position {2}")]
    Expected(String, String, usize),

    #[error("Unexpected end of expression")]
    UnexpectedEof,

    #[error("Token error: {0}")]
    TokenError(String),
}

/// The parser state.
pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse the token stream into a single expression AST.
    pub fn parse(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_or()?;
        // We should be at EOF now
        if !self.at_end() {
            let tok = self.peek();
            return Err(ParseError::UnexpectedToken(
                format!("{}", tok.kind),
                tok.span.0,
            ));
        }
        Ok(expr)
    }

    // ----- Helpers -----

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<&Token, ParseError> {
        let tok = self.peek();
        if std::mem::discriminant(&tok.kind) == std::mem::discriminant(expected) {
            Ok(self.advance())
        } else {
            Err(ParseError::Expected(
                format!("{}", expected),
                format!("{}", tok.kind),
                tok.span.0,
            ))
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    // ----- Grammar rules -----

    // or_expr = and_expr ( "or" and_expr )*
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.peek().kind == TokenKind::Or {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // and_expr = not_expr ( "and" not_expr )*
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while self.peek().kind == TokenKind::And {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    // not_expr = "not" not_expr | comparison
    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.peek().kind == TokenKind::Not {
            self.advance();
            let operand = self.parse_not()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }
        self.parse_comparison()
    }

    // comparison = addition ( ("==" | "!=" | "<" | ">" | "<=" | ">=" | "in") addition )*
    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_addition()?;

        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::BangEq => BinaryOp::Ne,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::LtEq => BinaryOp::Le,
                TokenKind::GtEq => BinaryOp::Ge,
                TokenKind::In => BinaryOp::In,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // addition = multiplication ( ("+" | "-") multiplication )*
    fn parse_addition(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplication()?;

        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // multiplication = unary ( ("*" | "/" | "%") unary )*
    fn parse_multiplication(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    // unary = "-" unary | postfix
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.peek().kind == TokenKind::Minus {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            });
        }
        self.parse_postfix()
    }

    // postfix = primary ( "." IDENT | "[" expr "]" | "(" args ")" )*
    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek().kind {
                TokenKind::Dot => {
                    self.advance();
                    // The field after dot
                    let tok = self.advance().clone();
                    let field = match &tok.kind {
                        TokenKind::Ident(name) => name.clone(),
                        // Allow keywords as field names (e.g., obj.in, obj.and)
                        TokenKind::And => "and".to_string(),
                        TokenKind::Or => "or".to_string(),
                        TokenKind::Not => "not".to_string(),
                        TokenKind::In => "in".to_string(),
                        TokenKind::True => "true".to_string(),
                        TokenKind::False => "false".to_string(),
                        TokenKind::Null => "null".to_string(),
                        _ => {
                            return Err(ParseError::Expected(
                                "identifier".to_string(),
                                format!("{}", tok.kind),
                                tok.span.0,
                            ));
                        }
                    };
                    expr = Expr::DotAccess {
                        object: Box::new(expr),
                        field,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_or()?;
                    self.expect(&TokenKind::RBracket)?;
                    expr = Expr::IndexAccess {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                TokenKind::LParen => {
                    // Only if the expression so far is an identifier (function name)
                    // or a dot-access chain (method-like call).
                    // For now we handle Ident -> FunctionCall transformation.
                    if let Expr::Ident(name) = expr {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect(&TokenKind::RParen)?;
                        expr = Expr::FunctionCall { name, args };
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    // args = ( expr ( "," expr )* )?
    fn parse_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_or()?);
        while self.peek().kind == TokenKind::Comma {
            self.advance();
            args.push(self.parse_or()?);
        }
        Ok(args)
    }

    // primary = INTEGER | FLOAT | STRING | "true" | "false" | "null"
    //         | IDENT | "(" expr ")" | "[" elements "]"
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Integer(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Literal(serde_json::json!(n)))
            }
            TokenKind::Float(f) => {
                let f = *f;
                self.advance();
                Ok(Expr::Literal(serde_json::json!(f)))
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal(serde_json::Value::String(s)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal(serde_json::json!(true)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal(serde_json::json!(false)))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Literal(serde_json::json!(null)))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Ident(name))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_or()?;
                self.expect(&TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    elements.push(self.parse_or()?);
                    while self.peek().kind == TokenKind::Comma {
                        self.advance();
                        // Allow trailing comma
                        if self.check(&TokenKind::RBracket) {
                            break;
                        }
                        elements.push(self.parse_or()?);
                    }
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(Expr::Array(elements))
            }
            _ => Err(ParseError::UnexpectedToken(
                format!("{}", tok.kind),
                tok.span.0,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tokenizer::Tokenizer;

    fn parse(input: &str) -> Expr {
        let tokens = Tokenizer::new(input).tokenize().unwrap();
        Parser::new(&tokens).parse().unwrap()
    }

    #[test]
    fn test_simple_add() {
        let ast = parse("2 + 3");
        assert_eq!(
            ast,
            Expr::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(Expr::Literal(serde_json::json!(2))),
                right: Box::new(Expr::Literal(serde_json::json!(3))),
            }
        );
    }

    #[test]
    fn test_precedence() {
        // 2 + 3 * 4 should parse as 2 + (3 * 4)
        let ast = parse("2 + 3 * 4");
        match ast {
            Expr::BinaryOp {
                op: BinaryOp::Add,
                right,
                ..
            } => {
                assert!(matches!(
                    *right,
                    Expr::BinaryOp {
                        op: BinaryOp::Mul,
                        ..
                    }
                ));
            }
            _ => panic!("Expected Add at top level"),
        }
    }

    #[test]
    fn test_function_call() {
        let ast = parse("length(arr)");
        assert_eq!(
            ast,
            Expr::FunctionCall {
                name: "length".to_string(),
                args: vec![Expr::Ident("arr".to_string())],
            }
        );
    }

    #[test]
    fn test_dot_access() {
        let ast = parse("obj.field.sub");
        assert_eq!(
            ast,
            Expr::DotAccess {
                object: Box::new(Expr::DotAccess {
                    object: Box::new(Expr::Ident("obj".to_string())),
                    field: "field".to_string(),
                }),
                field: "sub".to_string(),
            }
        );
    }

    #[test]
    fn test_array_literal() {
        let ast = parse("[1, 2, 3]");
        assert_eq!(
            ast,
            Expr::Array(vec![
                Expr::Literal(serde_json::json!(1)),
                Expr::Literal(serde_json::json!(2)),
                Expr::Literal(serde_json::json!(3)),
            ])
        );
    }

    #[test]
    fn test_bracket_access() {
        let ast = parse("arr[0]");
        assert_eq!(
            ast,
            Expr::IndexAccess {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::Literal(serde_json::json!(0))),
            }
        );
    }

    #[test]
    fn test_not_operator() {
        let ast = parse("not true");
        assert_eq!(
            ast,
            Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(Expr::Literal(serde_json::json!(true))),
            }
        );
    }

    #[test]
    fn test_in_operator() {
        let ast = parse("x in arr");
        assert_eq!(
            ast,
            Expr::BinaryOp {
                op: BinaryOp::In,
                left: Box::new(Expr::Ident("x".to_string())),
                right: Box::new(Expr::Ident("arr".to_string())),
            }
        );
    }

    #[test]
    fn test_complex_expression() {
        // Should parse without error
        let _ast = parse("length(items) > 3 and 5 in items");
    }

    #[test]
    fn test_chained_access() {
        // data.users[1].name
        let _ast = parse("data.users[1].name");
    }

    #[test]
    fn test_nested_function() {
        let _ast = parse("length(split(\"a,b,c\", \",\"))");
    }

    #[test]
    fn test_trailing_comma_in_array() {
        let ast = parse("[1, 2, 3,]");
        assert_eq!(
            ast,
            Expr::Array(vec![
                Expr::Literal(serde_json::json!(1)),
                Expr::Literal(serde_json::json!(2)),
                Expr::Literal(serde_json::json!(3)),
            ])
        );
    }
}
