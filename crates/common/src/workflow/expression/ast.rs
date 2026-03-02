//! # Expression AST
//!
//! Defines the abstract syntax tree nodes produced by the parser and consumed
//! by the evaluator.

use std::fmt;

/// A binary operator connecting two sub-expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    // Logical
    And,
    Or,
    // Membership
    In,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Mod => write!(f, "%"),
            BinaryOp::Eq => write!(f, "=="),
            BinaryOp::Ne => write!(f, "!="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Le => write!(f, "<="),
            BinaryOp::Ge => write!(f, ">="),
            BinaryOp::And => write!(f, "and"),
            BinaryOp::Or => write!(f, "or"),
            BinaryOp::In => write!(f, "in"),
        }
    }
}

/// A unary operator applied to a single sub-expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation: `-x`
    Neg,
    /// Logical negation: `not x`
    Not,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "not"),
        }
    }
}

/// An expression AST node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal JSON value: number, string, bool, or null.
    Literal(serde_json::Value),

    /// An array literal: `[expr, expr, ...]`
    Array(Vec<Expr>),

    /// A variable reference by name (e.g., `x`, `parameters`, `item`).
    Ident(String),

    /// Binary operation: `left op right`
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation: `op operand`
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Property access: `expr.field`
    DotAccess {
        object: Box<Expr>,
        field: String,
    },

    /// Index/bracket access: `expr[index_expr]`
    IndexAccess {
        object: Box<Expr>,
        index: Box<Expr>,
    },

    /// Function call: `name(arg1, arg2, ...)`
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
}
