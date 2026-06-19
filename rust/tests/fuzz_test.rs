//! Property-based / fuzz tests.
//!
//! These use the dependency-free seeded PRNG in `common` (so any failure is
//! reproducible from the fixed seed). They check *invariants* over large numbers
//! of generated inputs rather than fixed expected values:
//!   * the lexer always terminates at EOF and never panics on arbitrary bytes;
//!   * the parser never panics and always terminates on arbitrary token soup;
//!   * pretty-printing is idempotent: `parse∘to_string` is a fixed point;
//!   * the evaluator agrees with an independent wrapping-arithmetic oracle.
//!
//! A coverage-guided `cargo-fuzz`/libFuzzer harness could be layered on top
//! (nightly-only); this is the portable equivalent that runs under stable
//! `cargo test` everywhere.

mod common;
use common::Rng;

use monkey::ast::Node;
use monkey::eval::eval;
use monkey::lexer::Lexer;
use monkey::object::{Environment, Object};
use monkey::parser::Parser;
use monkey::token::TokenType;

const CHARSET: &[u8] = b"abcdefgxy_0123456789 \t\n+-*/<>=!(){}[],;:\".";

fn random_source(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.below(max_len as u64 + 1) as usize;
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        s.push(*rng.pick(CHARSET) as char);
    }
    s
}

#[test]
fn fuzz_lexer_always_reaches_eof() {
    let mut rng = Rng::new(0xF00D_CAFE);
    for _ in 0..2000 {
        let src = random_source(&mut rng, 60);
        let mut l = Lexer::new(&src);
        let cap = src.len() * 2 + 16;
        let mut saw_eof = false;
        for _ in 0..cap {
            if l.next_token().token_type == TokenType::Eof {
                saw_eof = true;
                break;
            }
        }
        assert!(saw_eof, "lexer did not reach EOF for input {:?}", src);
    }
}

#[test]
fn fuzz_parser_never_panics() {
    let mut rng = Rng::new(0x1234_5678_9ABC);
    for _ in 0..2000 {
        let src = random_source(&mut rng, 60);
        // The contract: parsing any input returns a Program and records errors
        // rather than panicking or looping forever.
        let mut p = Parser::new(Lexer::new(&src));
        let _program = p.parse_program();
        let _ = p.errors().len();
    }
}

// ---- pretty-print idempotence ----

fn gen_atom(rng: &mut Rng) -> String {
    match rng.below(3) {
        0 => rng.int_in(0, 999).to_string(),
        1 => (*rng.pick(&["a", "b", "foo", "bar", "x"])).to_string(),
        _ => (*rng.pick(&["true", "false"])).to_string(),
    }
}

fn gen_expr(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return gen_atom(rng);
    }
    match rng.below(6) {
        0 | 1 => gen_atom(rng),
        2 => {
            let op = *rng.pick(&["+", "-", "*", "/", "<", ">", "==", "!="]);
            format!(
                "({} {} {})",
                gen_expr(rng, depth - 1),
                op,
                gen_expr(rng, depth - 1)
            )
        }
        3 => {
            let op = *rng.pick(&["-", "!"]);
            format!("({}{})", op, gen_expr(rng, depth - 1))
        }
        4 => format!("({})", gen_expr(rng, depth - 1)),
        _ => {
            let n = rng.below(4);
            let elems: Vec<String> = (0..n).map(|_| gen_expr(rng, depth - 1)).collect();
            format!("[{}]", elems.join(", "))
        }
    }
}

#[test]
fn prop_pretty_print_is_idempotent() {
    let mut rng = Rng::new(0xABCD_0001);
    for _ in 0..1000 {
        let depth = rng.below(5) as u32;
        let src = gen_expr(&mut rng, depth);

        let mut p1 = Parser::new(Lexer::new(&src));
        let prog1 = p1.parse_program();
        assert!(
            p1.errors().is_empty(),
            "generated source did not parse: {:?} -> {:?}",
            src,
            p1.errors()
        );
        let s1 = prog1.to_string();

        let mut p2 = Parser::new(Lexer::new(&s1));
        let prog2 = p2.parse_program();
        assert!(p2.errors().is_empty(), "reparse failed for {:?}", s1);
        let s2 = prog2.to_string();

        assert_eq!(s1, s2, "pretty-print not idempotent (orig {:?})", src);
    }
}

// ---- evaluator vs. independent arithmetic oracle ----

enum IntE {
    Num(i64),
    Add(Box<IntE>, Box<IntE>),
    Sub(Box<IntE>, Box<IntE>),
    Mul(Box<IntE>, Box<IntE>),
    Neg(Box<IntE>),
}

impl IntE {
    /// Fully-parenthesised rendering that the parser will read back unambiguously.
    fn render(&self) -> String {
        match self {
            IntE::Num(n) => n.to_string(), // always >= 0 by construction
            IntE::Add(l, r) => format!("({} + {})", l.render(), r.render()),
            IntE::Sub(l, r) => format!("({} - {})", l.render(), r.render()),
            IntE::Mul(l, r) => format!("({} * {})", l.render(), r.render()),
            IntE::Neg(e) => format!("(-{})", e.render()),
        }
    }

    /// Oracle evaluation using Go-equivalent wrapping int64 arithmetic.
    fn eval(&self) -> i64 {
        match self {
            IntE::Num(n) => *n,
            IntE::Add(l, r) => l.eval().wrapping_add(r.eval()),
            IntE::Sub(l, r) => l.eval().wrapping_sub(r.eval()),
            IntE::Mul(l, r) => l.eval().wrapping_mul(r.eval()),
            IntE::Neg(e) => e.eval().wrapping_neg(),
        }
    }
}

fn gen_int_expr(rng: &mut Rng, depth: u32) -> IntE {
    if depth == 0 {
        return IntE::Num(rng.int_in(0, 999));
    }
    match rng.below(5) {
        0 => IntE::Num(rng.int_in(0, 999)),
        1 => IntE::Add(
            Box::new(gen_int_expr(rng, depth - 1)),
            Box::new(gen_int_expr(rng, depth - 1)),
        ),
        2 => IntE::Sub(
            Box::new(gen_int_expr(rng, depth - 1)),
            Box::new(gen_int_expr(rng, depth - 1)),
        ),
        3 => IntE::Mul(
            Box::new(gen_int_expr(rng, depth - 1)),
            Box::new(gen_int_expr(rng, depth - 1)),
        ),
        _ => IntE::Neg(Box::new(gen_int_expr(rng, depth - 1))),
    }
}

#[test]
fn prop_eval_matches_arithmetic_oracle() {
    let mut rng = Rng::new(0x0BADF00D);

    for _ in 0..1500 {
        let depth = rng.below(5) as u32; // up to ~ depth 4 trees
        let e = gen_int_expr(&mut rng, depth);
        let src = e.render();
        let expected = e.eval();

        let mut p = Parser::new(Lexer::new(&src));
        let program = p.parse_program();
        assert!(p.errors().is_empty(), "oracle source did not parse: {:?}", src);

        let env = Environment::new();
        match eval(&Node::Program(program), &env) {
            Some(Object::Integer(got)) => {
                assert_eq!(got, expected, "mismatch for {:?}", src)
            }
            other => panic!("expected Integer for {:?}, got {:?}", src, other),
        }
    }
}
