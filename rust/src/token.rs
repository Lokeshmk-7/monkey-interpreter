//! Translation of the Go `token` package (`token/token.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! Go models token types as `type Type string` with a set of `const` values
//! ("ILLEGAL", "+", "==", "FUNCTION", ...). That is a *stringly typed* enum.
//!
//! Rust mapping chosen: a real `enum TokenType`. Each variant carries no data;
//! the original Go string is recovered through [`TokenType::as_str`] /
//! [`std::fmt::Display`]. This is the idiomatic Rust replacement because:
//!   * `match` over the enum is exhaustive (the compiler catches a forgotten
//!     token kind), whereas `match`/`==` over raw strings is not;
//!   * illegal token strings become unrepresentable.
//!
//! Discarded alternative: keep `pub struct Type(String)` (a newtype) to stay
//! byte-for-byte identical to Go. Rejected because it throws away exhaustiveness
//! checking and lets arbitrary strings masquerade as token types — the very
//! thing Rust's type system is good at preventing.
//!
//! The exact Go strings are preserved in `as_str` because the parser embeds
//! them verbatim in user-facing error messages (e.g. "expected next token to be
//! IDENT, got = instead"), so I/O equivalence requires the same spellings.

use std::collections::HashMap;
use std::fmt;

/// `token.Type` in Go. See the module docs for why this is an enum.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TokenType {
    // Special
    Illegal,
    Eof,

    // Identifiers + literals
    Ident,
    Int,
    Float,
    String,

    // Operators
    Bang,
    Assign,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Lt,
    Gt,
    Eq,
    NotEq,

    // Delimiters
    Comma,
    Semicolon,
    Colon,

    // Brackets
    Lparen,
    Rparen,
    Lbrace,
    Rbrace,
    Lbracket,
    Rbracket,

    // Keywords
    Function,
    Let,
    True,
    False,
    If,
    Else,
    Return,
    Macro,
}

impl TokenType {
    /// Returns the exact string the Go `const` block assigned to this token
    /// type. Centralising the spellings here keeps error messages identical to
    /// the Go implementation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::Illegal => "ILLEGAL",
            TokenType::Eof => "EOF",
            TokenType::Ident => "IDENT",
            TokenType::Int => "INT",
            TokenType::Float => "FLOAT",
            TokenType::String => "STRING",
            TokenType::Bang => "!",
            TokenType::Assign => "=",
            TokenType::Plus => "+",
            TokenType::Minus => "-",
            TokenType::Asterisk => "*",
            TokenType::Slash => "/",
            TokenType::Lt => "<",
            TokenType::Gt => ">",
            TokenType::Eq => "==",
            TokenType::NotEq => "!=",
            TokenType::Comma => ",",
            TokenType::Semicolon => ";",
            TokenType::Colon => ":",
            TokenType::Lparen => "(",
            TokenType::Rparen => ")",
            TokenType::Lbrace => "{",
            TokenType::Rbrace => "}",
            TokenType::Lbracket => "[",
            TokenType::Rbracket => "]",
            TokenType::Function => "FUNCTION",
            TokenType::Let => "LET",
            TokenType::True => "TRUE",
            TokenType::False => "FALSE",
            TokenType::If => "IF",
            TokenType::Else => "ELSE",
            TokenType::Return => "RETURN",
            TokenType::Macro => "MACRO",
        }
    }
}

// Go relies on `fmt`'s `%s` printing the underlying string of `token.Type`.
// Implementing `Display` reproduces that so `format!("{}", tok_type)` matches.
impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Go's `token.Token` struct.
///
/// Note: Go's field is named `Type`, which collides with the Rust keyword-like
/// `type`. Renamed to `token_type` (the only deviation from the Go field name);
/// the field is otherwise a 1:1 translation. `Literal` -> `literal: String`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub literal: String,
}

impl Token {
    /// Convenience constructor (Go uses struct literals like
    /// `token.Token{Type: ..., Literal: ...}`; Rust prefers a helper to keep
    /// call sites short).
    pub fn new(token_type: TokenType, literal: impl Into<String>) -> Self {
        Token {
            token_type,
            literal: literal.into(),
        }
    }
}

/// Go keeps `var keywords = map[string]Type{...}`, a package-level mutable-looking
/// map that is in practice read-only. Rust mapping: build the map lazily inside
/// `lookup_ident`. A package-level `static` `HashMap` would require `OnceLock`
/// or a crate like `lazy_static`; since the table has 8 entries and
/// `lookup_ident` is not hot, a plain `match` is both cheaper and clearer.
///
/// I keep the `HashMap` form here only to mirror the Go data structure exactly;
/// see `lookup_ident` for the actual (match-based) lookup that I ship. It is
/// `pub` so the integration test (`tests/token_test.rs`) can guard the two
/// representations against drifting apart.
pub fn keywords() -> HashMap<&'static str, TokenType> {
    HashMap::from([
        ("fn", TokenType::Function),
        ("let", TokenType::Let),
        ("true", TokenType::True),
        ("false", TokenType::False),
        ("if", TokenType::If),
        ("else", TokenType::Else),
        ("return", TokenType::Return),
        ("macro", TokenType::Macro),
    ])
}

/// Translation of `token.LookupIdent`.
///
/// Go: looks the identifier up in the `keywords` map, returning the keyword type
/// or `IDENT`. Rust: a `match` over string slices — the idiomatic, allocation-
/// free equivalent of "map lookup with default". The `keywords()` HashMap above
/// documents the same table for readers comparing against Go.
pub fn lookup_ident(ident: &str) -> TokenType {
    match ident {
        "fn" => TokenType::Function,
        "let" => TokenType::Let,
        "true" => TokenType::True,
        "false" => TokenType::False,
        "if" => TokenType::If,
        "else" => TokenType::Else,
        "return" => TokenType::Return,
        "macro" => TokenType::Macro,
        _ => TokenType::Ident,
    }
}
