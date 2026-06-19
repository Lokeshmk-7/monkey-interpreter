//! Translation of the Go `parser` package (`parser/parser.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! ## Pratt-parser dispatch tables
//! Go stores parse functions in `map[token.Type]prefixParseFn` /
//! `infixParseFn`, where each value is a *method value* (`p.parseIdent`) bound
//! to the receiver. Rust mapping: **`match` dispatch** in [`Parser::parse_prefix`]
//! / [`Parser::parse_infix`].
//!
//! Discarded alternative: `HashMap<TokenType, fn(&mut Parser, ...) -> ...>`.
//! This is the literal translation, but it fights the borrow checker: every
//! parse function needs `&mut self`, yet the function would be looked up *from*
//! a map owned by `self` — you cannot hold `&self` (for the map) and `&mut self`
//! (for the call) at once without cloning the map each call. A bare function
//! pointer table sidesteps the closure-capture issue but still reads worse than
//! a `match`. `match` is the idiomatic Rust dispatch and the compiler checks it.
//!
//! ## Precedence constants
//! Go uses `iota`-numbered `const`s (`LOWEST`, `EQUALS`, ...). Rust mapping:
//! an `enum Precedence` deriving `PartialOrd`/`Ord`, so `precedence <
//! peek_precedence()` works on the variants directly. The variant declaration
//! order encodes the precedence ladder — clearer than magic integers.
//!
//! ## Nil returns
//! Go parse functions return `nil` (interface) on failure. Rust mapping:
//! `Option<...>`, with `?` to short-circuit. On the tested inputs this is
//! behaviourally identical; on malformed input the relevant error has already
//! been recorded in `errors` before the `None` propagates, which is all the
//! error tests assert.

use crate::ast::*;
use crate::lexer::Lexer;
use crate::token::{Token, TokenType};

/// Go's `iota` precedence ladder. `Ord`/`PartialOrd` derive from declaration
/// order, replacing the integer comparisons.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Precedence {
    Lowest,
    Equals,      // ==
    LessGreater, // > or <
    Sum,         // +
    Product,     // *
    Prefix,      // -X or !X
    Call,        // myFunc(X)
    Index,       // array[index]
}

/// Translation of the package-level `precedences` map. A `match` returning the
/// default `Lowest` reproduces "map lookup with default".
fn precedence_of(t: TokenType) -> Precedence {
    match t {
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::Lt | TokenType::Gt => Precedence::LessGreater,
        TokenType::Plus | TokenType::Minus => Precedence::Sum,
        TokenType::Slash | TokenType::Asterisk => Precedence::Product,
        TokenType::Lparen => Precedence::Call,
        TokenType::Lbracket => Precedence::Index,
        _ => Precedence::Lowest,
    }
}

/// Go's `parser.Parser`. The `prefixParseFns`/`infixParseFns` map fields are
/// gone (replaced by `match` dispatch); everything else maps 1:1. Owns the
/// `Lexer` by value (Go held the `lexer.Lexer` interface).
pub struct Parser {
    l: Lexer,
    errors: Vec<String>,
    cur_token: Token,
    peek_token: Token,
}

impl Parser {
    /// Translation of `parser.New`. Reads two tokens to prime `cur`/`peek`.
    pub fn new(mut l: Lexer) -> Parser {
        // Go primes both tokens with two `nextToken()` calls; we need initial
        // placeholders before the first read. The first `next_token()` moves the
        // (placeholder) peek into cur and reads a real peek; the second makes cur
        // real too — identical end state to Go.
        let cur = l.next_token();
        let peek = l.next_token();
        Parser {
            l,
            errors: Vec::new(),
            cur_token: cur,
            peek_token: peek,
        }
    }

    fn next_token(&mut self) {
        // `std::mem::replace` moves peek into cur without cloning.
        self.cur_token = std::mem::replace(&mut self.peek_token, self.l.next_token());
    }

    /// Go's `Errors()`.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    fn peek_error(&mut self, typ: TokenType) {
        let msg = format!(
            "expected next token to be {}, got {} instead",
            typ, self.peek_token.token_type
        );
        self.errors.push(msg);
    }

    fn cur_token_is(&self, typ: TokenType) -> bool {
        self.cur_token.token_type == typ
    }

    fn peek_token_is(&self, typ: TokenType) -> bool {
        self.peek_token.token_type == typ
    }

    fn expect_peek(&mut self, typ: TokenType) -> bool {
        if self.peek_token_is(typ) {
            self.next_token();
            true
        } else {
            self.peek_error(typ);
            false
        }
    }

    /// Translation of `ParseProgram`.
    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };

        while !self.cur_token_is(TokenType::Eof) {
            if let Some(stmt) = self.parse_statement() {
                program.statements.push(stmt);
            }
            self.next_token();
        }

        program
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.cur_token.token_type {
            TokenType::Let => self.parse_let_statement(),
            TokenType::Return => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_let_statement(&mut self) -> Option<Statement> {
        let token = self.cur_token.clone();

        if !self.expect_peek(TokenType::Ident) {
            return None;
        }

        let name = Ident {
            token: self.cur_token.clone(),
            value: self.cur_token.literal.clone(),
        };

        if !self.expect_peek(TokenType::Assign) {
            return None;
        }

        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        while self.peek_token_is(TokenType::Semicolon) {
            self.next_token();
        }

        Some(Statement::Let(LetStatement { token, name, value }))
    }

    fn parse_return_statement(&mut self) -> Option<Statement> {
        let token = self.cur_token.clone();

        self.next_token();

        let return_value = self.parse_expression(Precedence::Lowest)?;

        while self.peek_token_is(TokenType::Semicolon) {
            self.next_token();
        }

        Some(Statement::Return(ReturnStatement {
            token,
            return_value,
        }))
    }

    fn parse_expression_statement(&mut self) -> Option<Statement> {
        let token = self.cur_token.clone();
        let expression = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token_is(TokenType::Semicolon) {
            self.next_token();
        }

        Some(Statement::Expression(ExpressionStatement { token, expression }))
    }

    /// Translation of `parseExpression`. The prefix/infix `nil` checks become
    /// the `match` arms in `parse_prefix`/`has_infix`.
    fn parse_expression(&mut self, precedence: Precedence) -> Option<Expression> {
        let mut left_exp = self.parse_prefix()?;

        while !self.cur_token_is(TokenType::Semicolon) && precedence < self.peek_precedence() {
            if !has_infix(self.peek_token.token_type) {
                return Some(left_exp);
            }

            self.next_token();
            left_exp = self.parse_infix(left_exp)?;
        }

        Some(left_exp)
    }

    /// Prefix dispatch — replaces the `prefixParseFns` map. The `_` arm is Go's
    /// "no prefix parse function" error.
    fn parse_prefix(&mut self) -> Option<Expression> {
        match self.cur_token.token_type {
            TokenType::Ident => Some(self.parse_ident()),
            TokenType::Int => self.parse_integer_literal(),
            TokenType::Float => self.parse_float_literal(),
            TokenType::Bang | TokenType::Minus => self.parse_prefix_expression(),
            TokenType::True | TokenType::False => Some(self.parse_boolean()),
            TokenType::Lparen => self.parse_grouped_expression(),
            TokenType::If => self.parse_if_expression(),
            TokenType::Function => self.parse_function_literal(),
            TokenType::String => Some(self.parse_string_literal()),
            TokenType::Lbracket => self.parse_array_literal(),
            TokenType::Lbrace => self.parse_hash_literal(),
            TokenType::Macro => self.parse_macro_literal(),
            _ => {
                self.errors.push(format!(
                    "no prefix parse function for {} found",
                    self.cur_token.token_type
                ));
                None
            }
        }
    }

    /// Infix dispatch — replaces the `infixParseFns` map. Called with `cur_token`
    /// already advanced onto the operator (matching Go).
    fn parse_infix(&mut self, left: Expression) -> Option<Expression> {
        match self.cur_token.token_type {
            TokenType::Plus
            | TokenType::Minus
            | TokenType::Asterisk
            | TokenType::Slash
            | TokenType::Eq
            | TokenType::NotEq
            | TokenType::Lt
            | TokenType::Gt => self.parse_infix_expression(left),
            TokenType::Lparen => Some(self.parse_call_expression(left)),
            TokenType::Lbracket => self.parse_index_expression(left),
            // Unreachable: `has_infix` gates entry, but keep total for safety.
            _ => Some(left),
        }
    }

    fn parse_ident(&mut self) -> Expression {
        Expression::Ident(Ident {
            token: self.cur_token.clone(),
            value: self.cur_token.literal.clone(),
        })
    }

    fn parse_integer_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();
        match token.literal.parse::<i64>() {
            Ok(value) => Some(Expression::IntegerLiteral(IntegerLiteral { token, value })),
            Err(_) => {
                self.errors
                    .push(format!("could not parse \"{}\" as integer", token.literal));
                None
            }
        }
    }

    fn parse_float_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();
        match token.literal.parse::<f64>() {
            Ok(value) => Some(Expression::FloatLiteral(FloatLiteral { token, value })),
            Err(_) => {
                self.errors
                    .push(format!("could not parse \"{}\" as float", token.literal));
                None
            }
        }
    }

    fn parse_prefix_expression(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();
        let operator = self.cur_token.literal.clone();

        self.next_token();

        let right = self.parse_expression(Precedence::Prefix)?;
        Some(Expression::Prefix(PrefixExpression {
            token,
            operator,
            right: Box::new(right),
        }))
    }

    fn peek_precedence(&self) -> Precedence {
        precedence_of(self.peek_token.token_type)
    }

    fn cur_precedence(&self) -> Precedence {
        precedence_of(self.cur_token.token_type)
    }

    fn parse_infix_expression(&mut self, left: Expression) -> Option<Expression> {
        let token = self.cur_token.clone();
        let operator = self.cur_token.literal.clone();

        let prec = self.cur_precedence();

        self.next_token();

        let right = self.parse_expression(prec)?;
        Some(Expression::Infix(InfixExpression {
            token,
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }))
    }

    fn parse_boolean(&mut self) -> Expression {
        Expression::Boolean(Boolean {
            token: self.cur_token.clone(),
            value: self.cur_token_is(TokenType::True),
        })
    }

    fn parse_grouped_expression(&mut self) -> Option<Expression> {
        self.next_token();

        let expr = self.parse_expression(Precedence::Lowest)?;

        if !self.expect_peek(TokenType::Rparen) {
            return None;
        }

        Some(expr)
    }

    fn parse_if_expression(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();

        if !self.expect_peek(TokenType::Lparen) {
            return None;
        }

        self.next_token();
        let condition = self.parse_expression(Precedence::Lowest)?;

        if !self.expect_peek(TokenType::Rparen) {
            return None;
        }

        if !self.expect_peek(TokenType::Lbrace) {
            return None;
        }

        let consequence = self.parse_block_statement();

        let alternative = if self.peek_token_is(TokenType::Else) {
            self.next_token();

            if !self.expect_peek(TokenType::Lbrace) {
                return None;
            }

            Some(self.parse_block_statement())
        } else {
            None
        };

        Some(Expression::If(IfExpression {
            token,
            condition: Box::new(condition),
            consequence,
            alternative,
        }))
    }

    fn parse_block_statement(&mut self) -> BlockStatement {
        let token = self.cur_token.clone();
        let mut statements = Vec::new();

        self.next_token();

        while !self.cur_token_is(TokenType::Rbrace) && !self.cur_token_is(TokenType::Eof) {
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }

        BlockStatement { token, statements }
    }

    fn parse_function_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();

        if !self.expect_peek(TokenType::Lparen) {
            return None;
        }

        let parameters = self.parse_function_parameters()?;

        if !self.expect_peek(TokenType::Lbrace) {
            return None;
        }

        let body = self.parse_block_statement();

        Some(Expression::FunctionLiteral(FunctionLiteral {
            token,
            parameters,
            body,
        }))
    }

    fn parse_function_parameters(&mut self) -> Option<Vec<Ident>> {
        let mut idents = Vec::new();

        if self.peek_token_is(TokenType::Rparen) {
            self.next_token();
            return Some(idents);
        }

        self.next_token();

        idents.push(Ident {
            token: self.cur_token.clone(),
            value: self.cur_token.literal.clone(),
        });

        while self.peek_token_is(TokenType::Comma) {
            self.next_token();
            self.next_token();
            idents.push(Ident {
                token: self.cur_token.clone(),
                value: self.cur_token.literal.clone(),
            });
        }

        if !self.expect_peek(TokenType::Rparen) {
            return None;
        }

        Some(idents)
    }

    fn parse_expression_list(&mut self, end: TokenType) -> Option<Vec<Expression>> {
        let mut list = Vec::new();

        if self.peek_token_is(end) {
            self.next_token();
            return Some(list);
        }

        self.next_token();
        list.push(self.parse_expression(Precedence::Lowest)?);

        while self.peek_token_is(TokenType::Comma) {
            self.next_token();
            self.next_token();
            list.push(self.parse_expression(Precedence::Lowest)?);
        }

        if !self.expect_peek(end) {
            return None;
        }

        Some(list)
    }

    fn parse_call_expression(&mut self, function: Expression) -> Expression {
        let token = self.cur_token.clone();
        // Go stores nil args if the list parse fails; we use an empty Vec for
        // the (untested) failure path, keeping the node shape valid.
        let arguments = self
            .parse_expression_list(TokenType::Rparen)
            .unwrap_or_default();
        Expression::Call(CallExpression {
            token,
            function: Box::new(function),
            arguments,
        })
    }

    fn parse_string_literal(&mut self) -> Expression {
        Expression::StringLiteral(StringLiteral {
            token: self.cur_token.clone(),
            value: self.cur_token.literal.clone(),
        })
    }

    fn parse_array_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();
        let elements = self.parse_expression_list(TokenType::Rbracket)?;
        Some(Expression::ArrayLiteral(ArrayLiteral { token, elements }))
    }

    fn parse_index_expression(&mut self, left: Expression) -> Option<Expression> {
        let token = self.cur_token.clone();

        self.next_token();
        let index = self.parse_expression(Precedence::Lowest)?;

        if !self.expect_peek(TokenType::Rbracket) {
            return None;
        }

        Some(Expression::Index(IndexExpression {
            token,
            left: Box::new(left),
            index: Box::new(index),
        }))
    }

    fn parse_hash_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();
        let mut pairs = Vec::new();

        while !self.peek_token_is(TokenType::Rbrace) {
            self.next_token();
            let key = self.parse_expression(Precedence::Lowest)?;

            if !self.expect_peek(TokenType::Colon) {
                return None;
            }

            self.next_token();
            let value = self.parse_expression(Precedence::Lowest)?;
            pairs.push((key, value));

            if !self.peek_token_is(TokenType::Rbrace) && !self.expect_peek(TokenType::Comma) {
                return None;
            }
        }

        if !self.expect_peek(TokenType::Rbrace) {
            return None;
        }

        Some(Expression::HashLiteral(HashLiteral { token, pairs }))
    }

    fn parse_macro_literal(&mut self) -> Option<Expression> {
        let token = self.cur_token.clone();

        if !self.expect_peek(TokenType::Lparen) {
            return None;
        }

        let parameters = self.parse_function_parameters()?;

        if !self.expect_peek(TokenType::Lbrace) {
            return None;
        }

        let body = self.parse_block_statement();

        Some(Expression::MacroLiteral(MacroLiteral {
            token,
            parameters,
            body,
        }))
    }
}

/// Whether a token type has an infix parse function (replaces the
/// `infixParseFns[type] == nil` check in `parseExpression`).
fn has_infix(t: TokenType) -> bool {
    matches!(
        t,
        TokenType::Plus
            | TokenType::Minus
            | TokenType::Asterisk
            | TokenType::Slash
            | TokenType::Eq
            | TokenType::NotEq
            | TokenType::Lt
            | TokenType::Gt
            | TokenType::Lparen
            | TokenType::Lbracket
    )
}
