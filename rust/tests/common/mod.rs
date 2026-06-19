//! Shared helpers for the integration test crates.
//!
//! In Rust each file under `tests/` is compiled as its own test *binary*; this
//! `common/mod.rs` is included via `mod common;` and is therefore re-compiled
//! into each one. Items unused by a given binary would warn, so the whole module
//! allows dead code.
//!
//! These helpers only touch the crate's **public** API — which is the point of
//! moving the tests out of the source files: it proves the public surface is
//! sufficient to drive and observe the interpreter.
#![allow(dead_code)]

use monkey::ast::{Node, Program};
use monkey::eval::eval;
use monkey::lexer::Lexer;
use monkey::object::{Environment, Object};
use monkey::parser::Parser;

/// Parse and return both the program and the parser (to inspect `errors()`).
pub fn parse(input: &str) -> (Program, Parser) {
    let mut p = Parser::new(Lexer::new(input));
    let program = p.parse_program();
    (program, p)
}

/// Parse, asserting there were no parser errors.
pub fn parse_ok(input: &str) -> Program {
    let (program, p) = parse(input);
    assert!(
        p.errors().is_empty(),
        "unexpected parser errors for {:?}:\n{}",
        input,
        p.errors().join("\n")
    );
    program
}

/// Lex + parse + eval a program string (Go's `testEval`).
pub fn eval_input(input: &str) -> Option<Object> {
    let program = parse_ok(input);
    let env = Environment::new();
    eval(&Node::Program(program), &env)
}

// --- assertion helpers operating on `Option<Object>` (Go's `test*Object`) ---

pub fn expect_integer(obj: &Option<Object>, want: i64) {
    match obj {
        Some(Object::Integer(v)) => assert_eq!(*v, want, "wrong integer value"),
        other => panic!("object is not Integer({}). got={:?}", want, other),
    }
}

pub fn expect_float(obj: &Option<Object>, want: f64) {
    match obj {
        Some(Object::Float(v)) => assert_eq!(*v, want, "wrong float value"),
        other => panic!("object is not Float({}). got={:?}", want, other),
    }
}

pub fn expect_boolean(obj: &Option<Object>, want: bool) {
    match obj {
        Some(Object::Boolean(v)) => assert_eq!(*v, want, "wrong boolean value"),
        other => panic!("object is not Boolean({}). got={:?}", want, other),
    }
}

pub fn expect_string(obj: &Option<Object>, want: &str) {
    match obj {
        Some(Object::Str(v)) => assert_eq!(v, want, "wrong string value"),
        other => panic!("object is not String({:?}). got={:?}", want, other),
    }
}

pub fn expect_nil(obj: &Option<Object>) {
    assert!(
        matches!(obj, Some(Object::Nil)),
        "object is not Nil. got={:?}",
        obj
    );
}

pub fn expect_error(obj: &Option<Object>, want_msg: &str) {
    match obj {
        Some(Object::Error(msg)) => assert_eq!(msg, want_msg, "wrong error message"),
        other => panic!("object is not Error({:?}). got={:?}", want_msg, other),
    }
}

// ---------------------------------------------------------------------------
// Tiny deterministic PRNG (xorshift64*) for the property/fuzz tests.
//
// Dependency-free on purpose (the crate ships zero deps; see Cargo.toml). A
// fixed seed makes failures reproducible — a counterexample always reproduces.
// A real coverage-guided fuzzer (cargo-fuzz / libFuzzer) could be added as a
// separate harness on nightly; this in-`cargo test` generator is the portable,
// offline equivalent that runs everywhere CI runs.
// ---------------------------------------------------------------------------
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Seed must be non-zero for xorshift.
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..n` (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// Pick a reference to a random element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    /// Random `i64` in the inclusive range `[lo, hi]`.
    pub fn int_in(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo + 1) as u64;
        lo + (self.below(span) as i64)
    }
}
