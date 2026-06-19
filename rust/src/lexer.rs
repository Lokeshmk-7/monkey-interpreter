//! Translation of the Go `lexer` package (`lexer/lexer.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! Go defines a `Lexer` *interface* with a single method `NextToken`, and an
//! unexported `lexer` struct implementing it; `New` returns the interface.
//!
//! Rust mapping chosen: drop the interface, expose a concrete `pub struct Lexer`
//! with an inherent `next_token` method. The Go interface has exactly one
//! implementation and exists only to hide the struct fields — module privacy in
//! Rust achieves the same hiding without the indirection.
//!
//! Discarded alternative: a `pub trait Lexer { fn next_token(&mut self) -> Token; }`
//! plus a private impl. Rejected: it adds dynamic dispatch / generics for no
//! behavioural gain. (Could be reinstated trivially if a second lexer ever
//! existed.)
//!
//! ## Byte handling
//! Go indexes the input as raw bytes (`l.input[pos]`, `l.ch byte`) and slices it
//! by byte offset to build literals. To stay byte-for-byte identical (including
//! the multi-byte/UTF-8 edge cases the Go code would hit), the input is stored
//! as `Vec<u8>` and the "current char" as a `u8`, with `0` used as the
//! end-of-input sentinel exactly like Go's `l.ch = 0`.

use crate::token::{self, Token, TokenType};

/// Go's unexported `lexer` struct, surfaced directly (see module docs).
pub struct Lexer {
    input: Vec<u8>,
    /// current position in input (points to current char)
    position: usize,
    /// current reading position in input (after current char)
    read_position: usize,
    /// current char under examination (0 == EOF sentinel, mirroring Go)
    ch: u8,
}

impl Lexer {
    /// Translation of `lexer.New`. Go returns the `Lexer` interface; we return
    /// the concrete struct. `readChar()` is called once to prime `ch`, exactly
    /// as in Go.
    pub fn new(input: &str) -> Lexer {
        let mut l = Lexer {
            input: input.as_bytes().to_vec(),
            position: 0,
            read_position: 0,
            ch: 0,
        };
        l.read_char();
        l
    }

    fn read_char(&mut self) {
        // Go: `if l.readPosition >= len(l.input) { l.ch = 0 }`.
        if self.read_position >= self.input.len() {
            self.ch = 0;
        } else {
            self.ch = self.input[self.read_position];
        }
        self.position = self.read_position;
        self.read_position += 1;
    }

    /// Translation of `(*lexer).NextToken`. The Go `switch l.ch` becomes a Rust
    /// `match self.ch`. Byte literals (`b'='`) replace Go's rune/byte literals.
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        // skip comments
        if self.ch == b'/' && self.peek_char() == b'/' {
            self.skip_comment();
        }

        let tok: Token;
        match self.ch {
            b'=' => {
                if self.peek_char() == b'=' {
                    let ch = self.ch;
                    self.read_char();
                    // Go builds the literal "==" from two bytes.
                    let literal = String::from_utf8(vec![ch, self.ch]).unwrap();
                    tok = Token::new(TokenType::Eq, literal);
                } else {
                    tok = new_token(TokenType::Assign, self.ch);
                }
            }
            b'!' => {
                if self.peek_char() == b'=' {
                    let ch = self.ch;
                    self.read_char();
                    let literal = String::from_utf8(vec![ch, self.ch]).unwrap();
                    tok = Token::new(TokenType::NotEq, literal);
                } else {
                    tok = new_token(TokenType::Bang, self.ch);
                }
            }
            b';' => tok = new_token(TokenType::Semicolon, self.ch),
            b':' => tok = new_token(TokenType::Colon, self.ch),
            b'(' => tok = new_token(TokenType::Lparen, self.ch),
            b')' => tok = new_token(TokenType::Rparen, self.ch),
            b',' => tok = new_token(TokenType::Comma, self.ch),
            b'+' => tok = new_token(TokenType::Plus, self.ch),
            b'-' => tok = new_token(TokenType::Minus, self.ch),
            b'*' => tok = new_token(TokenType::Asterisk, self.ch),
            b'/' => tok = new_token(TokenType::Slash, self.ch),
            b'<' => tok = new_token(TokenType::Lt, self.ch),
            b'>' => tok = new_token(TokenType::Gt, self.ch),
            b'{' => tok = new_token(TokenType::Lbrace, self.ch),
            b'}' => tok = new_token(TokenType::Rbrace, self.ch),
            b'[' => tok = new_token(TokenType::Lbracket, self.ch),
            b']' => tok = new_token(TokenType::Rbracket, self.ch),
            b'"' => {
                // Go assigns Type/Literal fields separately; we build the token
                // once the string body is read.
                tok = Token::new(TokenType::String, self.read_string());
            }
            0 => {
                // EOF: Go sets Literal "" and Type EOF.
                tok = Token::new(TokenType::Eof, "");
            }
            _ => {
                if is_digit(self.ch) {
                    // Go `return l.readNumberToken()` — early return so the
                    // trailing `read_char()` below is skipped (the number reader
                    // already advanced past the literal).
                    return self.read_number_token();
                }

                if is_letter(self.ch) {
                    let literal = self.read_ident();
                    let token_type = token::lookup_ident(&literal);
                    // Early return for the same reason as numbers.
                    return Token::new(token_type, literal);
                }

                tok = new_token(TokenType::Illegal, self.ch);
            }
        }

        self.read_char();
        tok
    }

    fn skip_whitespace(&mut self) {
        while self.ch == b' ' || self.ch == b'\t' || self.ch == b'\n' || self.ch == b'\r' {
            self.read_char();
        }
    }

    fn skip_comment(&mut self) {
        // BUG-FIX vs Go (found by `tests/fuzz_test.rs`): the Go original is
        // `for l.ch != '\n' && l.ch != '\r' { l.readChar() }`. At end-of-input
        // `l.ch` becomes 0, which is neither '\n' nor '\r', so a `//` comment
        // that is not terminated by a newline (e.g. the last line of a file, or
        // a REPL line) makes Go loop forever. We additionally stop at the EOF
        // sentinel (`0`). This terminates on the pathological input while leaving
        // every newline-terminated comment (all the Go tests) unchanged.
        while self.ch != b'\n' && self.ch != b'\r' && self.ch != 0 {
            self.read_char();
        }
        self.skip_whitespace();
    }

    fn peek_char(&self) -> u8 {
        if self.read_position >= self.input.len() {
            0
        } else {
            self.input[self.read_position]
        }
    }

    fn read_string(&mut self) -> String {
        let position = self.position + 1;
        loop {
            self.read_char();
            if self.ch == b'"' || self.ch == 0 {
                break;
            }
        }
        // Go slices `l.input[position:l.position]`. `from_utf8_lossy` matches Go's
        // behaviour of treating the byte slice as a string.
        String::from_utf8_lossy(&self.input[position..self.position]).into_owned()
    }

    /// Translation of `(*lexer).read`, which takes a predicate `func(byte) bool`.
    /// Rust: a generic over a closure `F: Fn(u8) -> bool`. This is the direct
    /// equivalent of passing `isLetter`/`isDigit` as first-class functions.
    fn read<F: Fn(u8) -> bool>(&mut self, check_fn: F) -> String {
        let position = self.position;
        while check_fn(self.ch) {
            self.read_char();
        }
        String::from_utf8_lossy(&self.input[position..self.position]).into_owned()
    }

    fn read_ident(&mut self) -> String {
        self.read(is_letter)
    }

    fn read_number(&mut self) -> String {
        self.read(is_digit)
    }

    fn read_number_token(&mut self) -> Token {
        let int_part = self.read_number();
        if self.ch != b'.' {
            return Token::new(TokenType::Int, int_part);
        }

        self.read_char();
        let frac_part = self.read_number();
        Token::new(TokenType::Float, format!("{}.{}", int_part, frac_part))
    }
}

fn is_letter(ch: u8) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_uppercase() || ch == b'_'
}

fn is_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}

/// Translation of the package-level helper `newToken(tokenType, ch byte)`.
/// Go does `Literal: string(ch)` (one byte -> one-char string).
fn new_token(token_type: TokenType, ch: u8) -> Token {
    Token::new(token_type, (ch as char).to_string())
}
