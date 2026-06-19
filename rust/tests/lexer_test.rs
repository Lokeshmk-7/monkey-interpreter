//! Integration tests for the `lexer` module — the full ported
//! `lexer/lexer_test.go::TestNextToken`, plus extra edge-case coverage.

use monkey::lexer::Lexer;
use monkey::token::TokenType;

fn lex_all(input: &str) -> Vec<(TokenType, String)> {
    let mut l = Lexer::new(input);
    let mut out = Vec::new();
    loop {
        let tok = l.next_token();
        let is_eof = tok.token_type == TokenType::Eof;
        out.push((tok.token_type, tok.literal));
        if is_eof {
            break;
        }
    }
    out
}

#[test]
fn test_next_token() {
    let input = r#"
    let five = 5;
    let ten = 10;
    let add = fn(x, y) {
        x + y;
    };
    let result = add(five, ten);
    !-/*0;
    2 < 10 > 7;

    if (5 < 10) {
        return true;
    } else {
        return false;
    }

    10 == 10;
    10 != 9;

    "foobar";
    "foo bar";

    [1, 2];

    {"foo": "bar"};

    // comment
    let a = 1; // inline comment

    let b = 123.45;
    let c = 0.678;
    let d = 9.0;

    macro(x, y) { x + y; };
    "#;

    let expected: &[(TokenType, &str)] = &[
        (TokenType::Let, "let"),
        (TokenType::Ident, "five"),
        (TokenType::Assign, "="),
        (TokenType::Int, "5"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "ten"),
        (TokenType::Assign, "="),
        (TokenType::Int, "10"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "add"),
        (TokenType::Assign, "="),
        (TokenType::Function, "fn"),
        (TokenType::Lparen, "("),
        (TokenType::Ident, "x"),
        (TokenType::Comma, ","),
        (TokenType::Ident, "y"),
        (TokenType::Rparen, ")"),
        (TokenType::Lbrace, "{"),
        (TokenType::Ident, "x"),
        (TokenType::Plus, "+"),
        (TokenType::Ident, "y"),
        (TokenType::Semicolon, ";"),
        (TokenType::Rbrace, "}"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "result"),
        (TokenType::Assign, "="),
        (TokenType::Ident, "add"),
        (TokenType::Lparen, "("),
        (TokenType::Ident, "five"),
        (TokenType::Comma, ","),
        (TokenType::Ident, "ten"),
        (TokenType::Rparen, ")"),
        (TokenType::Semicolon, ";"),
        (TokenType::Bang, "!"),
        (TokenType::Minus, "-"),
        (TokenType::Slash, "/"),
        (TokenType::Asterisk, "*"),
        (TokenType::Int, "0"),
        (TokenType::Semicolon, ";"),
        (TokenType::Int, "2"),
        (TokenType::Lt, "<"),
        (TokenType::Int, "10"),
        (TokenType::Gt, ">"),
        (TokenType::Int, "7"),
        (TokenType::Semicolon, ";"),
        (TokenType::If, "if"),
        (TokenType::Lparen, "("),
        (TokenType::Int, "5"),
        (TokenType::Lt, "<"),
        (TokenType::Int, "10"),
        (TokenType::Rparen, ")"),
        (TokenType::Lbrace, "{"),
        (TokenType::Return, "return"),
        (TokenType::True, "true"),
        (TokenType::Semicolon, ";"),
        (TokenType::Rbrace, "}"),
        (TokenType::Else, "else"),
        (TokenType::Lbrace, "{"),
        (TokenType::Return, "return"),
        (TokenType::False, "false"),
        (TokenType::Semicolon, ";"),
        (TokenType::Rbrace, "}"),
        (TokenType::Int, "10"),
        (TokenType::Eq, "=="),
        (TokenType::Int, "10"),
        (TokenType::Semicolon, ";"),
        (TokenType::Int, "10"),
        (TokenType::NotEq, "!="),
        (TokenType::Int, "9"),
        (TokenType::Semicolon, ";"),
        (TokenType::String, "foobar"),
        (TokenType::Semicolon, ";"),
        (TokenType::String, "foo bar"),
        (TokenType::Semicolon, ";"),
        (TokenType::Lbracket, "["),
        (TokenType::Int, "1"),
        (TokenType::Comma, ","),
        (TokenType::Int, "2"),
        (TokenType::Rbracket, "]"),
        (TokenType::Semicolon, ";"),
        (TokenType::Lbrace, "{"),
        (TokenType::String, "foo"),
        (TokenType::Colon, ":"),
        (TokenType::String, "bar"),
        (TokenType::Rbrace, "}"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "a"),
        (TokenType::Assign, "="),
        (TokenType::Int, "1"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "b"),
        (TokenType::Assign, "="),
        (TokenType::Float, "123.45"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "c"),
        (TokenType::Assign, "="),
        (TokenType::Float, "0.678"),
        (TokenType::Semicolon, ";"),
        (TokenType::Let, "let"),
        (TokenType::Ident, "d"),
        (TokenType::Assign, "="),
        (TokenType::Float, "9.0"),
        (TokenType::Semicolon, ";"),
        (TokenType::Macro, "macro"),
        (TokenType::Lparen, "("),
        (TokenType::Ident, "x"),
        (TokenType::Comma, ","),
        (TokenType::Ident, "y"),
        (TokenType::Rparen, ")"),
        (TokenType::Lbrace, "{"),
        (TokenType::Ident, "x"),
        (TokenType::Plus, "+"),
        (TokenType::Ident, "y"),
        (TokenType::Semicolon, ";"),
        (TokenType::Rbrace, "}"),
        (TokenType::Semicolon, ";"),
        (TokenType::Eof, ""),
    ];

    let mut l = Lexer::new(input);
    for (i, (ty, lit)) in expected.iter().enumerate() {
        let tok = l.next_token();
        assert_eq!(tok.token_type, *ty, "tests[{}] type, tok={:?}", i, tok);
        assert_eq!(tok.literal, *lit, "tests[{}] literal, tok={:?}", i, tok);
    }
}

// ---- extra coverage beyond the Go test ----

#[test]
fn eof_is_idempotent() {
    let mut l = Lexer::new("");
    for _ in 0..5 {
        let tok = l.next_token();
        assert_eq!(tok.token_type, TokenType::Eof);
        assert_eq!(tok.literal, "");
    }
}

#[test]
fn illegal_characters() {
    let toks = lex_all("@#");
    assert_eq!(toks[0], (TokenType::Illegal, "@".to_string()));
    assert_eq!(toks[1], (TokenType::Illegal, "#".to_string()));
    assert_eq!(toks[2].0, TokenType::Eof);
}

#[test]
fn unterminated_string_reads_to_eof() {
    // Go's readString stops at '"' or 0; an unterminated string consumes the rest.
    let toks = lex_all("\"abc");
    assert_eq!(toks[0], (TokenType::String, "abc".to_string()));
    assert_eq!(toks[1].0, TokenType::Eof);
}

#[test]
fn comment_only_input() {
    // A line that is only a comment yields EOF.
    let toks = lex_all("// just a comment\n");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, TokenType::Eof);
}

#[test]
fn comment_at_eof_without_newline_terminates() {
    // Regression for the latent non-termination bug inherited from Go's
    // skipComment (found by the fuzzer): a comment with no trailing newline at
    // EOF must terminate, not loop forever.
    let toks = lex_all("5 // trailing comment with no newline");
    assert_eq!(toks[0], (TokenType::Int, "5".to_string()));
    assert_eq!(toks[1].0, TokenType::Eof);

    // And a file that is *only* an unterminated comment.
    let toks = lex_all("// eof");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].0, TokenType::Eof);
}

#[test]
fn adjacent_two_char_operators() {
    let toks = lex_all("==!=<>");
    let kinds: Vec<TokenType> = toks.iter().map(|t| t.0).collect();
    assert_eq!(
        kinds,
        vec![
            TokenType::Eq,
            TokenType::NotEq,
            TokenType::Lt,
            TokenType::Gt,
            TokenType::Eof
        ]
    );
}

#[test]
fn underscore_identifiers() {
    let toks = lex_all("_foo _ bar_baz");
    assert_eq!(toks[0], (TokenType::Ident, "_foo".to_string()));
    assert_eq!(toks[1], (TokenType::Ident, "_".to_string()));
    assert_eq!(toks[2], (TokenType::Ident, "bar_baz".to_string()));
}
