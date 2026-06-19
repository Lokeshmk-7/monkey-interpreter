//! Integration tests for the `parser` module — full port of
//! `parser/parser_test.go`, plus exact error-message coverage.

mod common;
use common::parse;

use monkey::ast::*;

/// Stands in for Go's `interface{}` expected values.
#[derive(Clone)]
enum Lit {
    Int(i64),
    Float(f64),
    Str(&'static str),
    Bool(bool),
}

fn check_parser_errors(errors: &[String]) {
    assert!(
        errors.is_empty(),
        "parser has {} errors: {:?}",
        errors.len(),
        errors
    );
}

fn expr_stmt(stmt: &Statement) -> &ExpressionStatement {
    match stmt {
        Statement::Expression(es) => es,
        other => panic!("not an expression statement: {:?}", other),
    }
}

fn test_integer_literal(expr: &Expression, value: i64) {
    match expr {
        Expression::IntegerLiteral(il) => {
            assert_eq!(il.value, value);
            assert_eq!(il.token.literal, value.to_string());
        }
        other => panic!("not an IntegerLiteral: {:?}", other),
    }
}

fn test_float_literal(expr: &Expression, value: f64) {
    match expr {
        Expression::FloatLiteral(fl) => assert_eq!(fl.value, value),
        other => panic!("not a FloatLiteral: {:?}", other),
    }
}

fn test_ident(expr: &Expression, value: &str) {
    match expr {
        Expression::Ident(id) => {
            assert_eq!(id.value, value);
            assert_eq!(id.token.literal, value);
        }
        other => panic!("not an Ident: {:?}", other),
    }
}

fn test_boolean_literal(expr: &Expression, value: bool) {
    match expr {
        Expression::Boolean(b) => {
            assert_eq!(b.value, value);
            assert_eq!(b.token.literal, value.to_string());
        }
        other => panic!("not a Boolean: {:?}", other),
    }
}

fn test_literal_expression(expr: &Expression, expected: &Lit) {
    match expected {
        Lit::Int(v) => test_integer_literal(expr, *v),
        Lit::Float(v) => test_float_literal(expr, *v),
        Lit::Str(v) => test_ident(expr, v),
        Lit::Bool(v) => test_boolean_literal(expr, *v),
    }
}

fn test_infix_expression(expr: &Expression, left: &Lit, operator: &str, right: &Lit) {
    match expr {
        Expression::Infix(ie) => {
            test_literal_expression(&ie.left, left);
            assert_eq!(ie.operator, operator);
            test_literal_expression(&ie.right, right);
        }
        other => panic!("not an InfixExpression: {:?}", other),
    }
}

#[test]
fn test_let_statements() {
    let tests: &[(&str, &str, Lit)] = &[
        ("let x = 5;", "x", Lit::Int(5)),
        ("let y = true;", "y", Lit::Bool(true)),
        ("let foobar = y;", "foobar", Lit::Str("y")),
    ];
    for (input, expected_ident, expected_value) in tests {
        let (program, p) = parse(input);
        assert_eq!(program.statements.len(), 1);
        check_parser_errors(p.errors());
        match &program.statements[0] {
            Statement::Let(ls) => {
                assert_eq!(ls.token.literal, "let");
                assert_eq!(ls.name.value, *expected_ident);
                assert_eq!(ls.name.token.literal, *expected_ident);
                test_literal_expression(&ls.value, expected_value);
            }
            other => panic!("not a LetStatement: {:?}", other),
        }
    }
}

#[test]
fn test_let_statement_errors() {
    for input in &["let = 5;", "let x = ;", "let x 1;"] {
        let (_program, p) = parse(input);
        assert!(
            !p.errors().is_empty(),
            "no errors despite invalid statement: {}",
            input
        );
    }
}

#[test]
fn test_return_statement() {
    let tests: &[(&str, Lit)] = &[
        ("return 5;", Lit::Int(5)),
        ("return true;", Lit::Bool(true)),
        ("return x;", Lit::Str("x")),
    ];
    for (input, expected) in tests {
        let (program, p) = parse(input);
        assert_eq!(program.statements.len(), 1);
        check_parser_errors(p.errors());
        match &program.statements[0] {
            Statement::Return(rs) => {
                let want = match expected {
                    Lit::Int(v) => v.to_string(),
                    Lit::Bool(v) => v.to_string(),
                    Lit::Str(v) => v.to_string(),
                    Lit::Float(v) => v.to_string(),
                };
                assert_eq!(rs.return_value.to_string(), want);
                assert_eq!(rs.token.literal, "return");
            }
            other => panic!("not a ReturnStatement: {:?}", other),
        }
    }
}

#[test]
fn test_identifier_expression() {
    let (program, p) = parse("foobar");
    check_parser_errors(p.errors());
    assert_eq!(program.statements.len(), 1);
    test_ident(&expr_stmt(&program.statements[0]).expression, "foobar");
}

#[test]
fn test_integer_expression() {
    let (program, p) = parse("5;");
    check_parser_errors(p.errors());
    assert_eq!(program.statements.len(), 1);
    test_integer_literal(&expr_stmt(&program.statements[0]).expression, 5);
}

#[test]
fn test_float_expression() {
    for (input, expected) in &[("12.34", 12.34_f64), ("0.56", 0.56), ("78.00", 78.00)] {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        assert_eq!(program.statements.len(), 1);
        test_float_literal(&expr_stmt(&program.statements[0]).expression, *expected);
    }
}

#[test]
fn test_parsing_prefix_expressions() {
    let tests: &[(&str, &str, Lit)] = &[
        ("!5;", "!", Lit::Int(5)),
        ("-15;", "-", Lit::Int(15)),
        ("!true", "!", Lit::Bool(true)),
        ("!false", "!", Lit::Bool(false)),
    ];
    for (input, operator, value) in tests {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        assert_eq!(program.statements.len(), 1);
        match &expr_stmt(&program.statements[0]).expression {
            Expression::Prefix(pe) => {
                assert_eq!(pe.operator, *operator);
                test_literal_expression(&pe.right, value);
            }
            other => panic!("not a PrefixExpression: {:?}", other),
        }
    }
}

#[test]
fn test_parsing_infix_expressions() {
    let tests: &[(&str, Lit, &str, Lit)] = &[
        ("5 + 5;", Lit::Int(5), "+", Lit::Int(5)),
        ("5 - 5;", Lit::Int(5), "-", Lit::Int(5)),
        ("5 * 5;", Lit::Int(5), "*", Lit::Int(5)),
        ("5 / 5;", Lit::Int(5), "/", Lit::Int(5)),
        ("5 > 5;", Lit::Int(5), ">", Lit::Int(5)),
        ("5 < 5;", Lit::Int(5), "<", Lit::Int(5)),
        ("5 == 5;", Lit::Int(5), "==", Lit::Int(5)),
        ("5 != 5;", Lit::Int(5), "!=", Lit::Int(5)),
        ("true == true", Lit::Bool(true), "==", Lit::Bool(true)),
        ("true != false", Lit::Bool(true), "!=", Lit::Bool(false)),
        ("false == false", Lit::Bool(false), "==", Lit::Bool(false)),
        ("5 + 5.1;", Lit::Int(5), "+", Lit::Float(5.1)),
        ("5.0 - 5.2;", Lit::Float(5.0), "-", Lit::Float(5.2)),
        ("5.3 * 5.4;", Lit::Float(5.3), "*", Lit::Float(5.4)),
        ("5.5 / 5.6;", Lit::Float(5.5), "/", Lit::Float(5.6)),
        ("5.7 > 5.8;", Lit::Float(5.7), ">", Lit::Float(5.8)),
        ("5.9 < 5;", Lit::Float(5.9), "<", Lit::Int(5)),
        ("5 == 5.0;", Lit::Int(5), "==", Lit::Float(5.0)),
        ("5.1 != 5.1;", Lit::Float(5.1), "!=", Lit::Float(5.1)),
    ];
    for (input, left, operator, right) in tests {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        assert_eq!(program.statements.len(), 1);
        test_infix_expression(
            &expr_stmt(&program.statements[0]).expression,
            left,
            operator,
            right,
        );
    }
}

#[test]
fn test_operator_precedence_parsing() {
    let tests: &[(&str, &str)] = &[
        ("-a * b", "((-a) * b)"),
        ("!-a", "(!(-a))"),
        ("a + b + c", "((a + b) + c)"),
        ("a + b - c", "((a + b) - c)"),
        ("a * b * c", "((a * b) * c)"),
        ("a * b / c", "((a * b) / c)"),
        ("a + b / c", "(a + (b / c))"),
        ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
        ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
        ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
        ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
        (
            "3 + 4 * 5 == 3 * 1 + 4 * 5",
            "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
        ),
        ("true", "true"),
        ("false", "false"),
        ("3 > 5 == false", "((3 > 5) == false)"),
        ("3 < 5 == true", "((3 < 5) == true)"),
        ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
        ("(5 + 5) * 2", "((5 + 5) * 2)"),
        ("2 / (5 + 5)", "(2 / (5 + 5))"),
        ("-(5 + 5)", "(-(5 + 5))"),
        ("!(true == true)", "(!(true == true))"),
        ("a + add(b * c) + d", "((a + add((b * c))) + d)"),
        (
            "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
            "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))",
        ),
        (
            "add(a + b + c * d / f + g)",
            "add((((a + b) + ((c * d) / f)) + g))",
        ),
        (
            "a * [1, 2, 3, 4][b * c] * d",
            "((a * ([1, 2, 3, 4][(b * c)])) * d)",
        ),
        (
            "add(a * b[2], b[1], 2 * [1, 2][1])",
            "add((a * (b[2])), (b[1]), (2 * ([1, 2][1])))",
        ),
    ];
    for (input, expected) in tests {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        assert_eq!(program.to_string(), *expected, "input={}", input);
    }
}

#[test]
fn test_boolean_expression() {
    for (input, expected) in &[("true", true), ("false", false)] {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        assert_eq!(program.statements.len(), 1);
        test_boolean_literal(&expr_stmt(&program.statements[0]).expression, *expected);
    }
}

#[test]
fn test_if_expression() {
    let (program, p) = parse("if (x < y) { x }");
    check_parser_errors(p.errors());
    assert_eq!(program.statements.len(), 1);
    match &expr_stmt(&program.statements[0]).expression {
        Expression::If(ie) => {
            test_infix_expression(&ie.condition, &Lit::Str("x"), "<", &Lit::Str("y"));
            assert_eq!(ie.consequence.statements.len(), 1);
            test_ident(&expr_stmt(&ie.consequence.statements[0]).expression, "x");
            assert!(ie.alternative.is_none());
        }
        other => panic!("not an IfExpression: {:?}", other),
    }
}

#[test]
fn test_if_else_expression() {
    let (program, p) = parse("if (x < y) { x } else { y }");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::If(ie) => {
            test_infix_expression(&ie.condition, &Lit::Str("x"), "<", &Lit::Str("y"));
            test_ident(&expr_stmt(&ie.consequence.statements[0]).expression, "x");
            let alt = ie.alternative.as_ref().expect("alternative present");
            test_ident(&expr_stmt(&alt.statements[0]).expression, "y");
        }
        other => panic!("not an IfExpression: {:?}", other),
    }
}

#[test]
fn test_function_literal_parsing() {
    let (program, p) = parse("fn(x, y) { x + y; }");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::FunctionLiteral(f) => {
            assert_eq!(f.parameters.len(), 2);
            test_literal_expression(&Expression::Ident(f.parameters[0].clone()), &Lit::Str("x"));
            test_literal_expression(&Expression::Ident(f.parameters[1].clone()), &Lit::Str("y"));
            assert_eq!(f.body.statements.len(), 1);
            test_infix_expression(
                &expr_stmt(&f.body.statements[0]).expression,
                &Lit::Str("x"),
                "+",
                &Lit::Str("y"),
            );
        }
        other => panic!("not a FunctionLiteral: {:?}", other),
    }
}

#[test]
fn test_function_parameter_parsing() {
    let tests: &[(&str, &[&str])] = &[
        ("fn() {}", &[]),
        ("fn(x) {};", &["x"]),
        ("fn(x, y, z) {};", &["x", "y", "z"]),
    ];
    for (input, expected) in tests {
        let (program, p) = parse(input);
        check_parser_errors(p.errors());
        match &expr_stmt(&program.statements[0]).expression {
            Expression::FunctionLiteral(f) => {
                assert_eq!(f.parameters.len(), expected.len());
                for (i, ident) in expected.iter().enumerate() {
                    test_literal_expression(
                        &Expression::Ident(f.parameters[i].clone()),
                        &Lit::Str(ident),
                    );
                }
            }
            other => panic!("not a FunctionLiteral: {:?}", other),
        }
    }
}

#[test]
fn test_call_function_parsing() {
    let (program, p) = parse("add(1, 2 * 3, 4 + 5);");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::Call(ce) => {
            test_ident(&ce.function, "add");
            assert_eq!(ce.arguments.len(), 3);
            test_literal_expression(&ce.arguments[0], &Lit::Int(1));
            test_infix_expression(&ce.arguments[1], &Lit::Int(2), "*", &Lit::Int(3));
            test_infix_expression(&ce.arguments[2], &Lit::Int(4), "+", &Lit::Int(5));
        }
        other => panic!("not a CallExpression: {:?}", other),
    }
}

#[test]
fn test_string_literal_expression() {
    let (program, p) = parse(r#""hello world";"#);
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::StringLiteral(sl) => assert_eq!(sl.value, "hello world"),
        other => panic!("not a StringLiteral: {:?}", other),
    }
}

#[test]
fn test_parsing_array_literals() {
    let (program, p) = parse("[1, 2 * 2, 3 + 3]");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::ArrayLiteral(al) => {
            assert_eq!(al.elements.len(), 3);
            test_integer_literal(&al.elements[0], 1);
            test_infix_expression(&al.elements[1], &Lit::Int(2), "*", &Lit::Int(2));
            test_infix_expression(&al.elements[2], &Lit::Int(3), "+", &Lit::Int(3));
        }
        other => panic!("not an ArrayLiteral: {:?}", other),
    }
}

#[test]
fn test_parsing_index_expressions() {
    let (program, p) = parse("myArray[1 + 1]");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::Index(ix) => {
            test_ident(&ix.left, "myArray");
            test_infix_expression(&ix.index, &Lit::Int(1), "+", &Lit::Int(1));
        }
        other => panic!("not an IndexExpression: {:?}", other),
    }
}

#[test]
fn test_parsing_hash_literals() {
    // string keys -> ints
    let (program, p) = parse(r#"{"one": 1, "two": 2, "three": 3}"#);
    check_parser_errors(p.errors());
    let want = [("one", 1_i64), ("two", 2), ("three", 3)];
    match &expr_stmt(&program.statements[0]).expression {
        Expression::HashLiteral(hl) => {
            assert_eq!(hl.pairs.len(), 3);
            for (k, v) in &hl.pairs {
                let key = match k {
                    Expression::StringLiteral(s) => s.value.clone(),
                    other => panic!("key not string: {:?}", other),
                };
                let exp = want.iter().find(|(s, _)| *s == key).unwrap().1;
                test_integer_literal(v, exp);
            }
        }
        other => panic!("not a HashLiteral: {:?}", other),
    }

    // empty
    let (program, p) = parse("{}");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::HashLiteral(hl) => assert_eq!(hl.pairs.len(), 0),
        other => panic!("not a HashLiteral: {:?}", other),
    }

    // integer keys
    let (program, p) = parse("{1: 1, 2: 2, 3: 3}");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::HashLiteral(hl) => {
            assert_eq!(hl.pairs.len(), 3);
            for (k, v) in &hl.pairs {
                let key = match k {
                    Expression::IntegerLiteral(i) => i.value,
                    other => panic!("key not int: {:?}", other),
                };
                test_integer_literal(v, key);
            }
        }
        other => panic!("not a HashLiteral: {:?}", other),
    }

    // boolean keys
    let (program, p) = parse("{true: 1, false: 2}");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::HashLiteral(hl) => {
            assert_eq!(hl.pairs.len(), 2);
            for (k, v) in &hl.pairs {
                let exp = match k {
                    Expression::Boolean(b) if b.value => 1,
                    Expression::Boolean(_) => 2,
                    other => panic!("key not bool: {:?}", other),
                };
                test_integer_literal(v, exp);
            }
        }
        other => panic!("not a HashLiteral: {:?}", other),
    }

    // expression values
    let (program, p) = parse(r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#);
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::HashLiteral(hl) => {
            assert_eq!(hl.pairs.len(), 3);
            for (k, v) in &hl.pairs {
                let key = match k {
                    Expression::StringLiteral(s) => s.value.clone(),
                    other => panic!("key not string: {:?}", other),
                };
                match key.as_str() {
                    "one" => test_infix_expression(v, &Lit::Int(0), "+", &Lit::Int(1)),
                    "two" => test_infix_expression(v, &Lit::Int(10), "-", &Lit::Int(8)),
                    "three" => test_infix_expression(v, &Lit::Int(15), "/", &Lit::Int(5)),
                    other => panic!("unexpected key {}", other),
                }
            }
        }
        other => panic!("not a HashLiteral: {:?}", other),
    }
}

#[test]
fn test_macro_literal_parsing() {
    let (program, p) = parse("macro(x, y) { x + y; }");
    check_parser_errors(p.errors());
    match &expr_stmt(&program.statements[0]).expression {
        Expression::MacroLiteral(m) => {
            assert_eq!(m.parameters.len(), 2);
            test_literal_expression(&Expression::Ident(m.parameters[0].clone()), &Lit::Str("x"));
            test_literal_expression(&Expression::Ident(m.parameters[1].clone()), &Lit::Str("y"));
            assert_eq!(m.body.statements.len(), 1);
            test_infix_expression(
                &expr_stmt(&m.body.statements[0]).expression,
                &Lit::Str("x"),
                "+",
                &Lit::Str("y"),
            );
        }
        other => panic!("not a MacroLiteral: {:?}", other),
    }
}

// ---- new: exact error messages ----

#[test]
fn exact_error_messages() {
    let (_p, p1) = parse("let x 5;");
    assert_eq!(
        p1.errors(),
        &["expected next token to be =, got INT instead".to_string()]
    );

    let (_p, p2) = parse("foobar @;");
    // '@' lexes to ILLEGAL, which has no prefix parse fn.
    assert!(p2
        .errors()
        .iter()
        .any(|e| e == "no prefix parse function for ILLEGAL found"));

    let (_p, p3) = parse("if x { y }");
    // The *first* error is the missing '(' after `if`. (The parser then recovers
    // and reports further errors while resyncing — Go behaves the same — so we
    // assert on the first, primary error rather than the whole list.)
    assert_eq!(
        p3.errors()[0],
        "expected next token to be (, got IDENT instead"
    );
}
