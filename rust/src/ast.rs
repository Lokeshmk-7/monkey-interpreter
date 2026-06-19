//! Translation of the Go `ast` package (`ast/ast.go` + `ast/modify.go`).
//!
//! # Go -> Rust mapping for this module (the central design decision)
//!
//! Go models the AST with three *interfaces* — `Node`, `Statement`,
//! `Expression` — and ~18 concrete pointer types (`*LetStatement`,
//! `*InfixExpression`, ...). Polymorphism is achieved by storing interface
//! values and recovering the concrete type with a *type switch*
//! (`switch n := node.(type) { case *LetStatement: ... }`).
//!
//! Rust mapping chosen: **algebraic data types**.
//!   * `enum Statement` — one variant per statement kind.
//!   * `enum Expression` — one variant per expression kind.
//!   * `enum Node` — the umbrella (`Program | Statement | Expression`) used by
//!     the generic [`modify`] walker and by `eval`, mirroring Go's `ast.Node`.
//!
//! Go's type switch becomes a Rust `match`, which the compiler checks for
//! exhaustiveness — translating the "marker method" pattern
//! (`statementNode()`/`expressionNode()`) into compiler-enforced membership.
//!
//! Discarded alternative: `Box<dyn Node>` trait objects with `Any`-based
//! downcasting. That is the *literal* translation of Go interfaces, but it
//! reintroduces runtime type errors (downcast can fail), loses exhaustiveness,
//! and needs `as_any()` boilerplate on every node. Enums are the idiomatic and
//! safer choice for a closed set of node kinds.
//!
//! ## Recursion / heap indirection
//! Go interface values are pointers, so recursion is implicitly boxed. In Rust,
//! a directly self-referential `enum` is infinitely sized, so the recursive
//! `Expression`-in-`Expression` fields (`Prefix.right`, `Infix.left/right`,
//! `If.condition`, `Index.left/index`, `Call.function`) are wrapped in `Box`.
//! Fields that recurse *through* a `Vec` (array elements, call args, block
//! statements) need no `Box` because `Vec` already heap-allocates.
//!
//! ## `HashLiteral.Pairs`
//! Go uses `map[Expression]Expression` (an Expression as a map key — legal in Go
//! because interface values are comparable by pointer). Rust mapping:
//! `Vec<(Expression, Expression)>`. Two reasons:
//!   1. `Expression` cannot be a `HashMap` key (it contains `f64`, so it is
//!      neither `Eq` nor `Hash`).
//!   2. It makes `String()` output *deterministic*. Go's map iteration order is
//!      randomised, so `(&HashLiteral).String()` is non-deterministic in Go;
//!      the `Vec` preserves source order. No test depends on the Go ordering, so
//!      this is a strict improvement that keeps behaviour well-defined.

use crate::token::Token;
use std::fmt;

// ===========================================================================
// Statement / Expression / Node enums
// ===========================================================================

/// Go's `ast.Statement` interface. Variants are the concrete `*XxxStatement`
/// types. `BlockStatement` lives here even though the Go source quirkily gives
/// it `expressionNode()` (rather than `statementNode()`); it only ever appears
/// as a concrete field (`If.consequence`, `Function.body`), never inside a
/// `Statement`/`Expression` interface slot, so its placement is immaterial.
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Let(LetStatement),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
    Block(BlockStatement),
}

/// Go's `ast.Expression` interface.
#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Ident(Ident),
    IntegerLiteral(IntegerLiteral),
    FloatLiteral(FloatLiteral),
    Prefix(PrefixExpression),
    Infix(InfixExpression),
    Boolean(Boolean),
    If(IfExpression),
    FunctionLiteral(FunctionLiteral),
    Call(CallExpression),
    StringLiteral(StringLiteral),
    ArrayLiteral(ArrayLiteral),
    Index(IndexExpression),
    HashLiteral(HashLiteral),
    MacroLiteral(MacroLiteral),
}

/// Go's top-level `ast.Node` interface. Used wherever Go passes around a bare
/// `ast.Node` (the [`modify`] walker, `eval`, and `object.Quote`).
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression),
}

// ===========================================================================
// Concrete node structs (1:1 with the Go structs)
// ===========================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LetStatement {
    pub token: Token,
    pub name: Ident,
    pub value: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
    pub token: Token,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReturnStatement {
    pub token: Token,
    pub return_value: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExpressionStatement {
    pub token: Token,
    pub expression: Expression,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntegerLiteral {
    pub token: Token,
    pub value: i64,
}

// f64 is `PartialEq` but not `Eq`, so this struct (and everything containing it)
// derives only `PartialEq`. Go compares floats with `==` too, so semantics match.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatLiteral {
    pub token: Token,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrefixExpression {
    pub token: Token,
    pub operator: String,
    pub right: Box<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InfixExpression {
    pub token: Token,
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Boolean {
    pub token: Token,
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfExpression {
    pub token: Token,
    pub condition: Box<Expression>,
    pub consequence: BlockStatement,
    // Go uses a nil `*BlockStatement` for "no else"; Rust's idiomatic nil is
    // `Option`.
    pub alternative: Option<BlockStatement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockStatement {
    pub token: Token,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionLiteral {
    pub token: Token,
    // Go: []*ast.Ident. Rust: owned `Vec<Ident>` (the pointers exist in Go only
    // for sharing/mutation; ownership is clearer here).
    pub parameters: Vec<Ident>,
    pub body: BlockStatement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallExpression {
    pub token: Token,
    pub function: Box<Expression>, // Ident or FunctionLiteral
    pub arguments: Vec<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StringLiteral {
    pub token: Token,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrayLiteral {
    pub token: Token,
    pub elements: Vec<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndexExpression {
    pub token: Token,
    pub left: Box<Expression>,
    pub index: Box<Expression>,
}

/// See module docs for why `pairs` is a `Vec` rather than a `HashMap`.
#[derive(Clone, Debug, PartialEq)]
pub struct HashLiteral {
    pub token: Token,
    pub pairs: Vec<(Expression, Expression)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacroLiteral {
    pub token: Token,
    pub parameters: Vec<Ident>,
    pub body: BlockStatement,
}

// ===========================================================================
// TokenLiteral() — Go's `Node.TokenLiteral`
// ===========================================================================
//
// Go puts `TokenLiteral()` on each concrete type via the `Node` interface. Rust
// mapping: inherent methods on the enums that dispatch to the held struct's
// token. The string-typed return matches Go.

impl Program {
    pub fn token_literal(&self) -> String {
        // Go: first statement's literal, or "" when empty.
        match self.statements.first() {
            Some(s) => s.token_literal(),
            None => String::new(),
        }
    }
}

impl Statement {
    pub fn token_literal(&self) -> String {
        match self {
            Statement::Let(s) => s.token.literal.clone(),
            Statement::Return(s) => s.token.literal.clone(),
            Statement::Expression(s) => s.token.literal.clone(),
            Statement::Block(s) => s.token.literal.clone(),
        }
    }
}

impl Expression {
    pub fn token_literal(&self) -> String {
        match self {
            Expression::Ident(e) => e.token.literal.clone(),
            Expression::IntegerLiteral(e) => e.token.literal.clone(),
            Expression::FloatLiteral(e) => e.token.literal.clone(),
            Expression::Prefix(e) => e.token.literal.clone(),
            Expression::Infix(e) => e.token.literal.clone(),
            Expression::Boolean(e) => e.token.literal.clone(),
            Expression::If(e) => e.token.literal.clone(),
            Expression::FunctionLiteral(e) => e.token.literal.clone(),
            Expression::Call(e) => e.token.literal.clone(),
            Expression::StringLiteral(e) => e.token.literal.clone(),
            Expression::ArrayLiteral(e) => e.token.literal.clone(),
            Expression::Index(e) => e.token.literal.clone(),
            Expression::HashLiteral(e) => e.token.literal.clone(),
            Expression::MacroLiteral(e) => e.token.literal.clone(),
        }
    }
}

impl Node {
    pub fn token_literal(&self) -> String {
        match self {
            Node::Program(p) => p.token_literal(),
            Node::Statement(s) => s.token_literal(),
            Node::Expression(e) => e.token_literal(),
        }
    }
}

// ===========================================================================
// String() — Go's `Node.String`, mapped to `std::fmt::Display`
// ===========================================================================
//
// Each Go `String()` becomes a `Display` impl. The exact byte output is
// preserved because the parser/eval tests compare `program.String()` against
// literal expected strings (e.g. "((-a) * b)").

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for s in &self.statements {
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for s in &self.statements {
            write!(f, "{}", s)?;
        }
        Ok(())
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Let(s) => {
                // "let <name> = <value>;"
                write!(f, "{} {} = {};", s.token.literal, s.name, s.value)
            }
            Statement::Return(s) => {
                write!(f, "{} {};", s.token.literal, s.return_value)
            }
            Statement::Expression(s) => write!(f, "{}", s.expression),
            Statement::Block(s) => write!(f, "{}", s),
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Ident(e) => write!(f, "{}", e.value),
            Expression::IntegerLiteral(e) => f.write_str(&e.token.literal),
            Expression::FloatLiteral(e) => f.write_str(&e.token.literal),
            Expression::Prefix(e) => write!(f, "({}{})", e.operator, e.right),
            Expression::Infix(e) => write!(f, "({} {} {})", e.left, e.operator, e.right),
            Expression::Boolean(e) => f.write_str(&e.token.literal),
            Expression::If(e) => {
                write!(f, "if{} {}", e.condition, e.consequence)?;
                if let Some(alt) = &e.alternative {
                    write!(f, "else {}", alt)?;
                }
                Ok(())
            }
            Expression::FunctionLiteral(e) => {
                let params: Vec<String> = e.parameters.iter().map(|p| p.to_string()).collect();
                write!(f, "{}({}) {}", e.token.literal, params.join(", "), e.body)
            }
            Expression::Call(e) => {
                let args: Vec<String> = e.arguments.iter().map(|a| a.to_string()).collect();
                write!(f, "{}({})", e.function, args.join(", "))
            }
            // Go's StringLiteral.String returns TokenLiteral() (the raw content).
            Expression::StringLiteral(e) => f.write_str(&e.token.literal),
            Expression::ArrayLiteral(e) => {
                let elems: Vec<String> = e.elements.iter().map(|el| el.to_string()).collect();
                write!(f, "[{}]", elems.join(", "))
            }
            Expression::Index(e) => write!(f, "({}[{}])", e.left, e.index),
            Expression::HashLiteral(e) => {
                let pairs: Vec<String> = e
                    .pairs
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{{{}}}", pairs.join(", "))
            }
            Expression::MacroLiteral(e) => {
                let params: Vec<String> = e.parameters.iter().map(|p| p.to_string()).collect();
                write!(f, "{}({}) {}", e.token.literal, params.join(", "), e.body)
            }
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Program(p) => write!(f, "{}", p),
            Node::Statement(s) => write!(f, "{}", s),
            Node::Expression(e) => write!(f, "{}", e),
        }
    }
}

// ===========================================================================
// Node conversions
// ===========================================================================
//
// Go's `modify`/`eval` freely pass `ast.Node` and recover concrete types with
// `.(Statement)`, `.(Expression)`, `.(*BlockStatement)`, `.(*Ident)`. Go's
// single-value type assertion *panics* on mismatch; these helpers reproduce
// that (a mismatch is a translator bug, not a user error, exactly as in Go).

impl Node {
    pub fn expect_statement(self) -> Statement {
        match self {
            Node::Statement(s) => s,
            other => panic!("expected Statement node, got {:?}", other),
        }
    }

    pub fn expect_expression(self) -> Expression {
        match self {
            Node::Expression(e) => e,
            other => panic!("expected Expression node, got {:?}", other),
        }
    }

    fn expect_block(self) -> BlockStatement {
        match self {
            Node::Statement(Statement::Block(b)) => b,
            other => panic!("expected BlockStatement node, got {:?}", other),
        }
    }

    fn expect_ident(self) -> Ident {
        match self {
            Node::Expression(Expression::Ident(i)) => i,
            other => panic!("expected Ident node, got {:?}", other),
        }
    }
}

// ===========================================================================
// Modify — translation of `ast/modify.go`
// ===========================================================================
//
// Go: `Modify(node Node, modifier ModifierFunc) Node` walks the tree, replacing
// children with `Modify(child, modifier)`, then returns `modifier(node)`.
//
// `ModifierFunc` is `func(Node) Node`. Rust mapping: `&mut dyn FnMut(Node) ->
// Node`. `FnMut` (not `Fn`) because the macro/quote modifiers capture and mutate
// the evaluation environment.
//
// Note (faithful to Go): only the node kinds Go's switch enumerates are
// recursed into. In particular `CallExpression` and `MacroLiteral` children are
// NOT recursed — the modifier is still invoked on those nodes, but their
// subtrees are left untouched. This is exactly what the macro expander relies
// on.

pub type ModifierFunc<'a> = dyn FnMut(Node) -> Node + 'a;

pub fn modify(node: Node, modifier: &mut ModifierFunc) -> Node {
    match node {
        Node::Program(mut p) => {
            for stmt in p.statements.iter_mut() {
                let modified = modify(Node::Statement(stmt.clone()), modifier).expect_statement();
                *stmt = modified;
            }
            modifier(Node::Program(p))
        }
        Node::Statement(s) => modify_statement(s, modifier),
        Node::Expression(e) => modify_expression(e, modifier),
    }
}

fn modify_statement(stmt: Statement, modifier: &mut ModifierFunc) -> Node {
    let modified = match stmt {
        Statement::Expression(mut es) => {
            es.expression =
                modify(Node::Expression(es.expression), modifier).expect_expression();
            Statement::Expression(es)
        }
        Statement::Block(mut bs) => {
            for s in bs.statements.iter_mut() {
                *s = modify(Node::Statement(s.clone()), modifier).expect_statement();
            }
            Statement::Block(bs)
        }
        Statement::Return(mut rs) => {
            rs.return_value =
                modify(Node::Expression(rs.return_value), modifier).expect_expression();
            Statement::Return(rs)
        }
        Statement::Let(mut ls) => {
            ls.value = modify(Node::Expression(ls.value), modifier).expect_expression();
            Statement::Let(ls)
        }
    };
    modifier(Node::Statement(modified))
}

fn modify_expression(expr: Expression, modifier: &mut ModifierFunc) -> Node {
    let modified = match expr {
        Expression::Infix(mut ie) => {
            ie.left =
                Box::new(modify(Node::Expression(*ie.left), modifier).expect_expression());
            ie.right =
                Box::new(modify(Node::Expression(*ie.right), modifier).expect_expression());
            Expression::Infix(ie)
        }
        Expression::Prefix(mut pe) => {
            pe.right =
                Box::new(modify(Node::Expression(*pe.right), modifier).expect_expression());
            Expression::Prefix(pe)
        }
        Expression::Index(mut ix) => {
            ix.left =
                Box::new(modify(Node::Expression(*ix.left), modifier).expect_expression());
            ix.index =
                Box::new(modify(Node::Expression(*ix.index), modifier).expect_expression());
            Expression::Index(ix)
        }
        Expression::If(mut ie) => {
            ie.condition =
                Box::new(modify(Node::Expression(*ie.condition), modifier).expect_expression());
            ie.consequence =
                modify(Node::Statement(Statement::Block(ie.consequence)), modifier).expect_block();
            if let Some(alt) = ie.alternative {
                ie.alternative =
                    Some(modify(Node::Statement(Statement::Block(alt)), modifier).expect_block());
            }
            Expression::If(ie)
        }
        Expression::FunctionLiteral(mut fl) => {
            for p in fl.parameters.iter_mut() {
                *p = modify(Node::Expression(Expression::Ident(p.clone())), modifier)
                    .expect_ident();
            }
            fl.body =
                modify(Node::Statement(Statement::Block(fl.body)), modifier).expect_block();
            Expression::FunctionLiteral(fl)
        }
        Expression::ArrayLiteral(mut al) => {
            for e in al.elements.iter_mut() {
                *e = modify(Node::Expression(e.clone()), modifier).expect_expression();
            }
            Expression::ArrayLiteral(al)
        }
        Expression::HashLiteral(mut hl) => {
            let mut new_pairs = Vec::with_capacity(hl.pairs.len());
            for (k, v) in hl.pairs.into_iter() {
                let nk = modify(Node::Expression(k), modifier).expect_expression();
                let nv = modify(Node::Expression(v), modifier).expect_expression();
                new_pairs.push((nk, nv));
            }
            hl.pairs = new_pairs;
            Expression::HashLiteral(hl)
        }
        // Default arm == Go's "no case matched": recurse into nothing, just call
        // the modifier. Covers Ident, IntegerLiteral, FloatLiteral, Boolean,
        // StringLiteral, Call, MacroLiteral.
        other => other,
    };
    modifier(Node::Expression(modified))
}
