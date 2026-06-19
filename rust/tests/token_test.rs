//! Integration tests for the `token` module (ported from + extended beyond
//! `token/token_test.go`, which in Go had no dedicated test file — these guard
//! `lookup_ident`, the keyword table, and the `TokenType` string mapping).

use monkey::token::{keywords, lookup_ident, Token, TokenType};

#[test]
fn lookup_matches_keyword_table() {
    // The shipped `lookup_ident` (match) must agree with the documented
    // `keywords` HashMap for every keyword, and fall back to IDENT otherwise.
    for (kw, ty) in keywords() {
        assert_eq!(lookup_ident(kw), ty, "keyword {:?}", kw);
    }
    assert_eq!(lookup_ident("foobar"), TokenType::Ident);
    assert_eq!(lookup_ident(""), TokenType::Ident);
    assert_eq!(lookup_ident("Let"), TokenType::Ident); // case-sensitive
    assert_eq!(lookup_ident("fnn"), TokenType::Ident);
}

#[test]
fn token_type_strings_match_go() {
    // The exact Go `const` strings; the parser embeds these in error messages.
    let cases: &[(TokenType, &str)] = &[
        (TokenType::Illegal, "ILLEGAL"),
        (TokenType::Eof, "EOF"),
        (TokenType::Ident, "IDENT"),
        (TokenType::Int, "INT"),
        (TokenType::Float, "FLOAT"),
        (TokenType::String, "STRING"),
        (TokenType::Bang, "!"),
        (TokenType::Assign, "="),
        (TokenType::Plus, "+"),
        (TokenType::Minus, "-"),
        (TokenType::Asterisk, "*"),
        (TokenType::Slash, "/"),
        (TokenType::Lt, "<"),
        (TokenType::Gt, ">"),
        (TokenType::Eq, "=="),
        (TokenType::NotEq, "!="),
        (TokenType::Comma, ","),
        (TokenType::Semicolon, ";"),
        (TokenType::Colon, ":"),
        (TokenType::Lparen, "("),
        (TokenType::Rparen, ")"),
        (TokenType::Lbrace, "{"),
        (TokenType::Rbrace, "}"),
        (TokenType::Lbracket, "["),
        (TokenType::Rbracket, "]"),
        (TokenType::Function, "FUNCTION"),
        (TokenType::Let, "LET"),
        (TokenType::True, "TRUE"),
        (TokenType::False, "FALSE"),
        (TokenType::If, "IF"),
        (TokenType::Else, "ELSE"),
        (TokenType::Return, "RETURN"),
        (TokenType::Macro, "MACRO"),
    ];
    for (ty, want) in cases {
        assert_eq!(ty.as_str(), *want);
        assert_eq!(format!("{}", ty), *want, "Display must equal as_str");
    }
}

#[test]
fn token_constructor_and_equality() {
    let a = Token::new(TokenType::Int, "5");
    let b = Token::new(TokenType::Int, "5".to_string());
    assert_eq!(a, b);
    assert_eq!(a.token_type, TokenType::Int);
    assert_eq!(a.literal, "5");
    assert_ne!(a, Token::new(TokenType::Int, "6"));
    assert_ne!(a, Token::new(TokenType::Float, "5"));
}
