//! Translation of the Go `main` package (`main.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! * `os.Args` -> `std::env::args`. Go's `len(os.Args) == 1` ("only the program
//!   name") corresponds to Rust's `args.len() == 1` (Rust also puts the binary
//!   name at index 0).
//! * `ioutil.ReadFile` -> `std::fs::read_to_string`.
//! * `error` returns -> `Result<(), String>`; the error is printed to stderr and
//!   the process exits with code 1, matching `fmt.Fprintln(os.Stderr, err)` +
//!   `os.Exit(1)`.

use monkey::ast::Node;
use monkey::eval::eval;
use monkey::lexer::Lexer;
use monkey::object::{Environment, Object};
use monkey::parser::Parser;
use monkey::repl;
use std::io;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Start the Monkey REPL when invoked with no script argument.
    if args.len() == 1 {
        println!("This is the Monkey programming language!");
        println!("Feel free to type in commands");
        let stdin = io::stdin();
        let stdout = io::stdout();
        repl::start(stdin.lock(), stdout.lock());
        return;
    }

    // Run a Monkey script.
    if let Err(err) = run_program(&args[1]) {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}

fn run_program(filename: &str) -> Result<(), String> {
    let data = std::fs::read_to_string(filename)
        .map_err(|e| format!("could not read {}: {}", filename, e))?;

    let mut p = Parser::new(Lexer::new(&data));
    let program = p.parse_program();
    if !p.errors().is_empty() {
        return Err(p.errors()[0].clone());
    }

    let env = Environment::new();
    let result = eval(&Node::Program(program), &env);

    match result {
        // Go returns early on `*object.Nil` without printing.
        Some(Object::Nil) => Ok(()),
        Some(obj) => {
            println!("{}", obj.inspect());
            Ok(())
        }
        // Go's untyped-nil result would panic in `Inspect()`; we treat the
        // (only-on-empty/let-terminated programs) `None` case as "no output".
        None => Ok(()),
    }
}
