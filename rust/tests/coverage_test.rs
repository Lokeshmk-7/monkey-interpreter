//! Additional coverage beyond the ported Go tests: object `Inspect()` output
//! for compound values, recursion, higher-order functions, and error paths the
//! Go suite did not exercise directly.

mod common;
use common::*;

use monkey::object::Object;

fn inspect(input: &str) -> String {
    eval_input(input).expect("expected a value").inspect()
}

#[test]
fn function_inspect() {
    assert_eq!(inspect("fn(x, y) { x + y; }"), "fn(x, y) {\n(x + y)\n}");
    assert_eq!(inspect("fn() { 5 }"), "fn() {\n5\n}");
}

#[test]
fn array_inspect_via_eval() {
    assert_eq!(inspect("[1, 2 * 2, \"x\" + \"y\"]"), "[1, 4, xy]");
    assert_eq!(inspect("[]"), "[]");
}

#[test]
fn builtin_inspect() {
    // A builtin referenced by name evaluates to the Builtin object.
    match eval_input("len") {
        Some(Object::Builtin(_)) => {}
        other => panic!("expected Builtin, got {:?}", other),
    }
    assert_eq!(inspect("len"), "builtin function");
}

#[test]
fn nested_data_structures() {
    expect_integer(&eval_input("[[1, 2], [3, 4]][1][0]"), 3);
    expect_integer(&eval_input(r#"{"a": [10, 20, 30]}["a"][2]"#), 30);
    expect_string(
        &eval_input(r#"{"outer": {"inner": "deep"}}["outer"]["inner"]"#),
        "deep",
    );
}

#[test]
fn recursion_fibonacci() {
    let prog = "
    let fib = fn(n) {
        if (n < 2) { return n; }
        fib(n - 1) + fib(n - 2);
    };
    fib(10);
    ";
    expect_integer(&eval_input(prog), 55);
}

#[test]
fn recursion_factorial() {
    let prog = "
    let fact = fn(n) { if (n == 0) { 1 } else { n * fact(n - 1) } };
    fact(6);
    ";
    expect_integer(&eval_input(prog), 720);
}

#[test]
fn higher_order_map_and_reduce() {
    let prelude = "
    let map = fn(arr, f) {
        let iter = fn(arr, acc) {
            if (len(arr) == 0) { acc }
            else { iter(rest(arr), push(acc, f(first(arr)))); }
        };
        iter(arr, []);
    };
    let reduce = fn(arr, init, f) {
        let iter = fn(arr, result) {
            if (len(arr) == 0) { result }
            else { iter(rest(arr), f(result, first(arr))); }
        };
        iter(arr, init);
    };
    let sum = fn(arr) { reduce(arr, 0, fn(a, b) { a + b }); };
    let double = fn(x) { x * 2 };
    ";

    expect_integer(&eval_input(&format!("{} sum([1, 2, 3, 4, 5]);", prelude)), 15);
    assert_eq!(
        inspect(&format!("{} map([1, 2, 3], double);", prelude)),
        "[2, 4, 6]"
    );
}

#[test]
fn closures_keep_independent_state() {
    let prog = "
    let newCounter = fn() {
        let count = 0;
        fn() { count };
    };
    let c = newCounter();
    c();
    ";
    // count is not mutated (Monkey `let` doesn't reassign), so this is 0.
    expect_integer(&eval_input(prog), 0);
}

#[test]
fn let_with_valueless_if_is_nil() {
    // `if (false) { 1 }` with no else evaluates to NilValue; bound to x.
    expect_nil(&eval_input("let x = if (false) { 1 }; x"));
}

#[test]
fn additional_error_messages() {
    // calling a non-function
    expect_error(&eval_input("let x = 5; x(3);"), "not a function: Integer");
    // indexing an unsupported type
    expect_error(&eval_input("5[0]"), "index operator not supported: Integer");
    // array indexed by a non-integer
    expect_error(&eval_input("[1, 2][true]"), "index operator not supported: Array");
    // unknown prefix on float handled, but minus on boolean is an error
    expect_error(&eval_input("-true"), "unknown operator: -Boolean");
    // error short-circuits the rest of the program
    expect_error(&eval_input("5 + true; 5;"), "type mismatch: Integer + Boolean");
}

#[test]
fn integer_division_truncates_toward_zero() {
    expect_integer(&eval_input("7 / 2"), 3);
    expect_integer(&eval_input("-7 / 2"), -3);
}

#[test]
fn integer_overflow_wraps_like_go() {
    // i64::MAX is 9223372036854775807; +1 wraps to i64::MIN, matching Go int64.
    expect_integer(
        &eval_input("9223372036854775807 + 1"),
        i64::MIN,
    );
}
