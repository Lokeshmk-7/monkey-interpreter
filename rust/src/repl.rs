//! Translation of the Go `repl` package (`repl/repl.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! * `Start(in io.Reader, out io.Writer)` -> `start<R: BufRead, W: Write>`. Go's
//!   `io.Reader`/`io.Writer` interfaces map to the `std::io` traits; making
//!   `start` generic over them preserves the "inject any reader/writer"
//!   testability the Go signature affords.
//! * `bufio.NewScanner(in)` line iteration -> `BufRead::read_line` (a `read` of
//!   0 bytes is EOF, mirroring `scanner.Scan() == false`).
//! * The persistent `env` + `macroEnv` carry over between iterations exactly as
//!   in Go (the REPL keeps state across lines).

use crate::eval::{define_macros, eval, expand_macros};
use crate::ast::Node;
use crate::lexer::Lexer;
use crate::object::Environment;
use crate::parser::Parser;
use std::io::{BufRead, Write};

const PROMPT: &str = ">> ";

/// Go's `repl.Start`.
pub fn start<R: BufRead, W: Write>(mut reader: R, mut writer: W) {
    let env = Environment::new();
    let macro_env = Environment::new();

    loop {
        // Go prints the prompt via `fmt.Print`; writing it to `out` is
        // equivalent when `out` is stdout (as `main` uses).
        let _ = write!(writer, "{}", PROMPT);
        let _ = writer.flush();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return, // EOF
            Ok(_) => {}
            Err(_) => return,
        }

        // `scanner.Text()` yields the line without its trailing newline.
        let line = line.trim_end_matches(['\n', '\r']);

        let mut p = Parser::new(Lexer::new(line));
        let mut program = p.parse_program();
        if !p.errors().is_empty() {
            print_parser_errors(&mut writer, p.errors());
            continue;
        }

        // Process macros.
        define_macros(&mut program, &macro_env);
        let expanded = expand_macros(Node::Program(program), &macro_env);

        // Evaluate AST.
        match eval(&expanded, &env) {
            None => continue,
            Some(obj) => {
                let _ = writeln!(writer, "{}", obj.inspect());
            }
        }
    }
}

fn print_parser_errors<W: Write>(writer: &mut W, errors: &[String]) {
    for msg in errors {
        let _ = writeln!(writer, "{}", msg);
    }
}
