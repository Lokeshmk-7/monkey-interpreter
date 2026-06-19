//! Integration tests for the `eval` module — full port of `eval/eval_test.go`.

mod common;
use common::*;

use monkey::object::Object;

#[test]
fn eval_integer_expression() {
    let tests: &[(&str, i64)] = &[
        ("5", 5),
        ("10", 10),
        ("-5", -5),
        ("-10", -10),
        ("5 + 5 + 5 + 5 - 10", 10),
        ("2 * 2 * 2 * 2 * 2", 32),
        ("-50 + 100 + -50", 0),
        ("5 * 2 + 10", 20),
        ("5 + 2 * 10", 25),
        ("20 + 2 * -10", 0),
        ("50 / 2 * 2 + 10", 60),
        ("3 * 3 * 3 + 10", 37),
        ("3 * (3 * 3) + 10", 37),
        ("(5 + 10 * 2 + 15 / 3) * 2 + -10", 50),
    ];
    for (input, expected) in tests {
        expect_integer(&eval_input(input), *expected);
    }
}

#[test]
fn eval_float_expression() {
    let tests: &[(&str, f64)] = &[
        ("12.34", 12.34),
        ("0.56", 0.56),
        ("78.00", 78.00),
        ("-12.34", -12.34),
        ("-0.56", -0.56),
        ("-78.00", -78.00),
        ("(5 + 10.0 * 2.5 + 15.0 / 3) * 2.1 + -10.1", 63.4),
    ];
    for (input, expected) in tests {
        expect_float(&eval_input(input), *expected);
    }
}

#[test]
fn eval_boolean_expression() {
    let tests: &[(&str, bool)] = &[
        ("true", true),
        ("false", false),
        ("1 < 2", true),
        ("1 > 2", false),
        ("1 < 1", false),
        ("1 > 1", false),
        ("1 == 1", true),
        ("1 != 1", false),
        ("1 == 2", false),
        ("1 != 2", true),
        ("true == true", true),
        ("false == false", true),
        ("true == false", false),
        ("true != false", true),
        ("false != true", true),
        ("(1 < 2) == true", true),
        ("(1 < 2) == false", false),
        ("(1 > 2) == true", false),
        ("(1 > 2) == false", true),
        (r#""hello" == "hello""#, true),
        (r#""hello" == "world""#, false),
        (r#""foo" != "bar""#, true),
        (r#""foo" != "foo""#, false),
    ];
    for (input, expected) in tests {
        expect_boolean(&eval_input(input), *expected);
    }
}

#[test]
fn bang_operator() {
    let tests: &[(&str, bool)] = &[
        ("!true", false),
        ("!false", true),
        ("!5", false),
        ("!!true", true),
        ("!!false", false),
        ("!!5", true),
    ];
    for (input, expected) in tests {
        expect_boolean(&eval_input(input), *expected);
    }
}

#[test]
fn if_else_expressions() {
    let tests: &[(&str, Option<i64>)] = &[
        ("if (true) { 10 }", Some(10)),
        ("if (false) { 10 }", None),
        ("if (1) { 10 }", Some(10)),
        ("if (1 < 2) { 10 }", Some(10)),
        ("if (1 > 2) { 10 }", None),
        ("if (1 < 2) { 10 } else { 20 }", Some(10)),
        ("if (1 > 2) { 10 } else { 20 }", Some(20)),
    ];
    for (input, expected) in tests {
        let got = eval_input(input);
        match expected {
            Some(i) => expect_integer(&got, *i),
            None => expect_nil(&got),
        }
    }
}

#[test]
fn return_statements() {
    let tests: &[(&str, i64)] = &[
        ("return 10;", 10),
        ("return 10; 9;", 10),
        ("return 2 * 5; 9;", 10),
        ("9; return 2 * 5; 11;", 10),
        ("if (10 > 1) { if (10 > 1) { return 10; } return 1; }", 10),
    ];
    for (input, expected) in tests {
        expect_integer(&eval_input(input), *expected);
    }
}

#[test]
fn error_handling() {
    let tests: &[(&str, &str)] = &[
        ("5 + true;", "type mismatch: Integer + Boolean"),
        ("5 + true; 5;", "type mismatch: Integer + Boolean"),
        ("-true", "unknown operator: -Boolean"),
        ("true + false", "unknown operator: Boolean + Boolean"),
        ("5; true + false; 5", "unknown operator: Boolean + Boolean"),
        (
            "if (10 > 1) { true + false; }",
            "unknown operator: Boolean + Boolean",
        ),
        (
            "if (10 > 1) { if (10 > 1) { return true + false; } return 1; }",
            "unknown operator: Boolean + Boolean",
        ),
        ("foobar", "identifier not found: foobar"),
        (r#""Hello" - "World""#, "unknown operator: String - String"),
        (r#"1.5 + "World""#, "unknown operator: Float + String"),
        (r#"{[1, 2]: "Monkey"}"#, "unusable as hash key: Array"),
        (
            r#"{"name": "Monkey"}[fn(x) { x }]"#,
            "unusable as hash key: Function",
        ),
    ];
    for (input, expected) in tests {
        expect_error(&eval_input(input), expected);
    }
}

#[test]
fn let_statements() {
    let tests: &[(&str, i64)] = &[
        ("let a = 5; a;", 5),
        ("let a = 5 * 5; a;", 25),
        ("let a = 5; let b = a; b;", 5),
        ("let a = 5; let b = a; let c = a + b + 5; c;", 15),
    ];
    for (input, expected) in tests {
        expect_integer(&eval_input(input), *expected);
    }
}

#[test]
fn function_object() {
    match eval_input("fn(x) { x + 2; }") {
        Some(Object::Function(f)) => {
            assert_eq!(f.parameters.len(), 1);
            assert_eq!(f.parameters[0].to_string(), "x");
            assert_eq!(f.body.to_string(), "(x + 2)");
        }
        other => panic!("not a Function: {:?}", other),
    }
}

#[test]
fn function_application() {
    let tests: &[(&str, i64)] = &[
        ("let identity = fn(x) { x; }; identity(5);", 5),
        ("let identity = fn(x) { return x; }; identity(5);", 5),
        ("let double = fn(x) { x * 2; }; double(5);", 10),
        ("let add = fn(x, y) { x + y; }; add(5, 5);", 10),
        ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", 20),
        ("fn(x) { x; }(5);", 5),
    ];
    for (input, expected) in tests {
        expect_integer(&eval_input(input), *expected);
    }
}

#[test]
fn closures() {
    let input = "
    let newAdder = fn(x) { fn(y) { x + y }; };
    let addTwo = newAdder(2);
    addTwo(2);
    ";
    expect_integer(&eval_input(input), 4);
}

#[test]
fn string_literal_and_concat() {
    expect_string(&eval_input(r#""Hello World!";"#), "Hello World!");
    expect_string(&eval_input(r#""Hello" + " " + "World!";"#), "Hello World!");
}

#[test]
fn builtin_functions() {
    enum Want {
        Int(i64),
        Err(&'static str),
        Arr(Vec<i64>),
        Nil,
    }
    use Want::*;

    let tests: Vec<(&str, Want)> = vec![
        (r#"len("")"#, Int(0)),
        (r#"len("four")"#, Int(4)),
        (r#"len("hello world")"#, Int(11)),
        (r#"len("hello" + " " + "world")"#, Int(11)),
        (r#"len(1)"#, Err("argument to `len` not supported, got Integer")),
        (
            r#"len("one", "two")"#,
            Err("wrong number of arguments. want=1, got=2"),
        ),
        ("len([])", Int(0)),
        ("len([1])", Int(1)),
        ("len([1, 1 + 2 * 3, true])", Int(3)),
        ("first([])", Nil),
        ("first([1])", Int(1)),
        ("first([1, 2])", Int(1)),
        ("first(1)", Err("argument to `first` must be Array, got Integer")),
        ("last([])", Nil),
        ("last([1])", Int(1)),
        ("last([1, 2])", Int(2)),
        ("last(1)", Err("argument to `last` must be Array, got Integer")),
        ("rest([])", Nil),
        ("rest([1])", Arr(vec![])),
        ("rest([1, 2, 3])", Arr(vec![2, 3])),
        ("rest(1)", Err("argument to `last` must be Array, got Integer")),
        ("push([], 1)", Arr(vec![1])),
        ("push([1, 2], 3)", Arr(vec![1, 2, 3])),
        ("push([])", Err("wrong number of arguments. want=2, got=1")),
        (
            "push(1, 2)",
            Err("first argument to `push` must be Array, got Integer"),
        ),
        ("puts(1)", Nil),
    ];

    for (input, want) in tests {
        let got = eval_input(input);
        match want {
            Int(v) => expect_integer(&got, v),
            Err(msg) => expect_error(&got, msg),
            Nil => expect_nil(&got),
            Arr(expected) => match &got {
                Some(Object::Array(elements)) => {
                    assert_eq!(elements.len(), expected.len());
                    for (i, e) in elements.iter().enumerate() {
                        expect_integer(&Some(e.clone()), expected[i]);
                    }
                }
                other => panic!("not an Array: {:?}", other),
            },
        }
    }
}

#[test]
fn array_literals() {
    match eval_input("[1, 2 * 2, 3 + 3]") {
        Some(Object::Array(elements)) => {
            assert_eq!(elements.len(), 3);
            expect_integer(&Some(elements[0].clone()), 1);
            expect_integer(&Some(elements[1].clone()), 4);
            expect_integer(&Some(elements[2].clone()), 6);
        }
        other => panic!("not an Array: {:?}", other),
    }
}

#[test]
fn array_index_expressions() {
    let tests: &[(&str, Option<i64>)] = &[
        ("[1, 2, 3][0]", Some(1)),
        ("[1, 2, 3][1]", Some(2)),
        ("[1, 2, 3][2]", Some(3)),
        ("let i = 0; [1][i]", Some(1)),
        ("[1, 2, 3][1 + 1]", Some(3)),
        ("let arr = [1, 2, 3]; arr[2];", Some(3)),
        ("let arr = [1, 2, 3]; arr[0] + arr[1] + arr[2];", Some(6)),
        ("let arr = [1, 2, 3]; let i = arr[0]; arr[i]", Some(2)),
        ("[1, 2, 3][3]", None),
        ("[1, 2, 3][-1]", None),
    ];
    for (input, expected) in tests {
        let got = eval_input(input);
        match expected {
            Some(i) => expect_integer(&got, *i),
            None => expect_nil(&got),
        }
    }
}

#[test]
fn hash_literals() {
    let input = r#"
    let two = "two";
    {
        "one": 10 - 9,
        two: 1 + 1,
        "thr" + "ee": 6 / 2,
        4: 4,
        true: 5,
        false: 6
    };
    "#;

    let hash = match eval_input(input) {
        Some(Object::Hash(h)) => h,
        other => panic!("not a Hash: {:?}", other),
    };

    let expected = vec![
        (Object::Str("one".into()).hash_key().unwrap(), 1_i64),
        (Object::Str("two".into()).hash_key().unwrap(), 2),
        (Object::Str("three".into()).hash_key().unwrap(), 3),
        (Object::Integer(4).hash_key().unwrap(), 4),
        (Object::Boolean(true).hash_key().unwrap(), 5),
        (Object::Boolean(false).hash_key().unwrap(), 6),
    ];

    assert_eq!(hash.pairs.len(), expected.len());
    for (key, value) in expected {
        let pair = hash.pairs.get(&key).expect("no pair for key");
        expect_integer(&Some(pair.value.clone()), value);
    }
}

#[test]
fn hash_index_expressions() {
    let tests: &[(&str, Option<i64>)] = &[
        (r#"{"foo": 2 + 3}["foo"]"#, Some(5)),
        (r#"{"foo": 5}["bar"]"#, None),
        (r#"let key = "foo"; {"foo": 5}[key]"#, Some(5)),
        (r#"{}["foo"]"#, None),
        ("{5: 5}[5]", Some(5)),
        ("{true: 5}[true]", Some(5)),
        ("{false: 5}[false]", Some(5)),
    ];
    for (input, expected) in tests {
        let got = eval_input(input);
        match expected {
            Some(i) => expect_integer(&got, *i),
            None => expect_nil(&got),
        }
    }
}
