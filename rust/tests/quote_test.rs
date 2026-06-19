//! Integration tests for quote/unquote — ports `eval/quote_test.go` and the
//! `TestQuote` cases from `eval/eval_test.go`.

mod common;
use common::eval_input;

use monkey::object::Object;

fn assert_quote(input: &str, want: &str) {
    match eval_input(input) {
        Some(Object::Quote(node)) => assert_eq!(node.to_string(), want, "input={}", input),
        other => panic!("expected Quote for {:?}, got {:?}", input, other),
    }
}

#[test]
fn quote() {
    assert_quote("quote(5)", "5");
    assert_quote("quote(foobar)", "foobar");
    assert_quote("quote(foobar + barfoo)", "(foobar + barfoo)");
}

#[test]
fn quote_unquote() {
    let tests: &[(&str, &str)] = &[
        ("quote(unquote(4))", "4"),
        ("quote(unquote(4 + 4))", "8"),
        ("quote(8 + unquote(4 + 4))", "(8 + 8)"),
        ("quote(unquote(4 + 4) + 8)", "(8 + 8)"),
        ("let foobar = 8; quote(foobar)", "foobar"),
        ("let foobar = 8; quote(unquote(foobar))", "8"),
        ("quote(unquote(true))", "true"),
        ("quote(unquote(true == false))", "false"),
        ("quote(unquote(quote(4 + 4)))", "(4 + 4)"),
        (
            "let quotedInfixExpr = quote(4 + 4); quote(unquote(4 + 4) + unquote(quotedInfixExpr))",
            "(8 + (4 + 4))",
        ),
    ];
    for (input, want) in tests {
        assert_quote(input, want);
    }
}
