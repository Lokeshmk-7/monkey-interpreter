//! # Monkey interpreter — Rust transpilation
//!
//! A line-by-line, semantics-preserving Rust port of the Go Monkey interpreter
//! (`skatsuta/monkey-interpreter`, itself based on Thorsten Ball's *Writing an
//! Interpreter in Go*, extended with float literals, comments and a macro
//! system).
//!
//! ## Module ↔ Go package map
//!
//! | Rust module | Go package | Go file(s) |
//! |-------------|-----------|------------|
//! | [`token`]   | `token`   | `token/token.go` |
//! | [`lexer`]   | `lexer`   | `lexer/lexer.go` |
//! | [`ast`]     | `ast`     | `ast/ast.go`, `ast/modify.go` |
//! | [`object`]  | `object`  | `object/object.go`, `object/environment.go` |
//! | [`parser`]  | `parser`  | `parser/parser.go` |
//! | [`eval`]    | `eval`    | `eval/eval.go`, `builtin.go`, `quote.go`, `macro.go` |
//! | [`repl`]    | `repl`    | `repl/repl.go` |
//!
//! The bottom-up dependency order (each module only depends on those above it)
//! is exactly the translation order described in `docs/translation_notes.md`.
//!
//! ## Cross-cutting idiom translations
//! * Go interfaces with a closed implementation set (`Node`, `Statement`,
//!   `Expression`, `Object`, `Type`) → Rust `enum`s matched exhaustively.
//! * Go type switches → Rust `match`.
//! * Shared, mutable evaluation state (`Environment`, closures) → `Rc<RefCell<…>>`.
//! * Go's nullable `Object` interface returned from `Eval` → `Option<Object>`.
//! * Maps of bound method values (Pratt parser) → `match` dispatch.

pub mod ast;
pub mod eval;
pub mod lexer;
pub mod object;
pub mod parser;
pub mod repl;
pub mod token;
