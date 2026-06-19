//! True end-to-end integration tests: whole programs through lex→parse→eval,
//! and the REPL driven via in-memory reader/writer (exercising `repl::start`,
//! including parser-error reporting and the macro pipeline that `main`/file mode
//! does not run).

mod common;
use common::eval_input;

/// Run the REPL over `input` (as if typed at the prompt) and capture all output.
/// `&[u8]` implements `BufRead`; `&mut Vec<u8>` implements `Write`.
fn run_repl(input: &str) -> String {
    let mut out: Vec<u8> = Vec::new();
    monkey::repl::start(input.as_bytes(), &mut out);
    String::from_utf8(out).expect("repl output is valid utf-8")
}

#[test]
fn end_to_end_program_value() {
    // A self-contained program whose final expression is the result.
    let prog = "
    let twice = fn(f, x) { f(f(x)); };
    let addThree = fn(x) { x + 3; };
    twice(addThree, 7);
    ";
    let result = eval_input(prog).expect("value").inspect();
    assert_eq!(result, "13");
}

#[test]
fn end_to_end_string_building() {
    let prog = r#"
    let greet = fn(name) { "Hello, " + name + "!"; };
    greet("Monkey");
    "#;
    assert_eq!(eval_input(prog).unwrap().inspect(), "Hello, Monkey!");
}

#[test]
fn repl_evaluates_expressions() {
    let out = run_repl("let add = fn(a, b) { a + b };\nadd(3, 4);\n");
    assert!(out.contains("7"), "output was: {:?}", out);
    // The prompt is emitted before each read.
    assert!(out.contains(">> "), "no prompt in: {:?}", out);
}

#[test]
fn repl_reports_runtime_errors_as_values() {
    let out = run_repl("foobar;\n");
    assert!(
        out.contains("Error: identifier not found: foobar"),
        "output was: {:?}",
        out
    );
}

#[test]
fn repl_reports_parser_errors() {
    let out = run_repl("let;\n");
    assert!(
        out.contains("expected next token to be IDENT, got ; instead"),
        "output was: {:?}",
        out
    );
}

#[test]
fn repl_macro_pipeline() {
    // Macros only work through the REPL (define + expand + eval).
    let line = "let unless = macro(c, a, b) { quote(if (!(unquote(c))) { unquote(a) } else { unquote(b) }); };";
    let out = run_repl(&format!("{}\nunless(true, 1, 2);\n", line));
    // !(true) -> false -> else branch -> 2
    assert!(out.contains("2"), "output was: {:?}", out);
}

#[test]
fn repl_state_persists_across_lines() {
    let out = run_repl("let x = 10;\nlet y = x * 2;\ny + 5;\n");
    assert!(out.contains("25"), "output was: {:?}", out);
}

#[test]
fn repl_eof_terminates_cleanly() {
    // Empty input => immediate EOF => just the first prompt, no panic.
    let out = run_repl("");
    assert_eq!(out, ">> ");
}
