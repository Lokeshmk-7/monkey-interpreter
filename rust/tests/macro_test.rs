//! Integration tests for the macro system — ports `eval/macro_test.go`.

mod common;
use common::parse_ok;

use monkey::ast::Node;
use monkey::eval::{define_macros, expand_macros};
use monkey::object::{Environment, Object};

#[test]
fn define_macros_registers_and_strips() {
    let input = "
    let num = 1;
    let func = fn(x, y) { x + y };
    let mymacro = macro(x, y) { x + y; };
    ";

    let env = Environment::new();
    let mut program = parse_ok(input);
    define_macros(&mut program, &env);

    // The two non-macro lets remain; the macro let is stripped.
    assert_eq!(program.statements.len(), 2);
    assert!(env.borrow().get("num").is_none());
    assert!(env.borrow().get("func").is_none());

    // Bind first so the `Ref` from `borrow()` is dropped before the match body.
    let mymacro = env.borrow().get("mymacro").expect("mymacro defined");
    match mymacro {
        Object::Macro(m) => {
            assert_eq!(m.parameters.len(), 2);
            assert_eq!(m.parameters[0].to_string(), "x");
            assert_eq!(m.parameters[1].to_string(), "y");
            assert_eq!(m.body.to_string(), "(x + y)");
        }
        other => panic!("not a Macro: {:?}", other),
    }
}

#[test]
fn expand_macros_rewrites_calls() {
    let tests: &[(&str, &str)] = &[
        (
            "let infixExpr = macro() { quote(1 + 2); }; infixExpr();",
            "(1 + 2)",
        ),
        (
            "let reverse = macro(a, b) { quote(unquote(b) - unquote(a)); }; reverse(2 + 2, 10 - 5);",
            "(10 - 5) - (2 + 2)",
        ),
        (
            "
            let unless = macro(condition, consequence, altenative) {
              quote(
                if (!(unquote(condition))) {
                    unquote(consequence)
                } else {
                    unquote(altenative)
                }
              );
            };
            unless(10 > 5, puts(\"not greater\"), puts(\"greater\"));
            ",
            "
            if (!(10 > 5)) {
                puts(\"not greater\")
            } else {
                puts(\"greater\")
            }
            ",
        ),
    ];

    for (input, want) in tests {
        let mut program = parse_ok(input);
        let env = Environment::new();
        define_macros(&mut program, &env);
        let got = expand_macros(Node::Program(program), &env).to_string();
        // Compare against the parsed-then-restringified expectation (normalises
        // whitespace), exactly as the Go test does.
        let want_normalised = parse_ok(want).to_string();
        assert_eq!(got, want_normalised, "input={}", input);
    }
}
