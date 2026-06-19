//! Integration tests for the `ast` module — ports `ast/ast_test.go` (String)
//! and `ast/modify_test.go` (Modify), plus extra Display/TokenLiteral coverage.

use monkey::ast::*;
use monkey::token::{Token, TokenType};

fn dummy() -> Token {
    Token::new(TokenType::Illegal, "")
}

fn int_lit_valued(value: i64, literal: &str) -> Expression {
    Expression::IntegerLiteral(IntegerLiteral {
        token: Token::new(TokenType::Int, literal),
        value,
    })
}

fn one() -> Expression {
    int_lit_valued(1, "1")
}

// After modification, value is 2 but the token literal stays "1" — matching
// Go's in-place mutation of only the `Value` field.
fn two() -> Expression {
    int_lit_valued(2, "1")
}

fn turn_one_into_two(node: Node) -> Node {
    if let Node::Expression(Expression::IntegerLiteral(mut il)) = node {
        if il.value == 1 {
            il.value = 2;
        }
        return Node::Expression(Expression::IntegerLiteral(il));
    }
    node
}

fn run_modify(input: Node) -> Node {
    let mut m = turn_one_into_two;
    modify(input, &mut m)
}

#[test]
fn test_string() {
    let program = Program {
        statements: vec![Statement::Let(LetStatement {
            token: Token::new(TokenType::Let, "let"),
            name: Ident {
                token: Token::new(TokenType::Ident, "myVar"),
                value: "myVar".to_string(),
            },
            value: Expression::Ident(Ident {
                token: Token::new(TokenType::Ident, "anotherVar"),
                value: "anotherVar".to_string(),
            }),
        })],
    };
    assert_eq!(program.to_string(), "let myVar = anotherVar;");
}

#[test]
fn test_modify() {
    // bare expression
    assert_eq!(run_modify(Node::Expression(one())), Node::Expression(two()));

    // program
    assert_eq!(
        run_modify(Node::Program(Program {
            statements: vec![Statement::Expression(ExpressionStatement {
                token: dummy(),
                expression: one()
            })],
        })),
        Node::Program(Program {
            statements: vec![Statement::Expression(ExpressionStatement {
                token: dummy(),
                expression: two()
            })],
        })
    );

    // infix (left)
    assert_eq!(
        run_modify(Node::Expression(Expression::Infix(InfixExpression {
            token: dummy(),
            left: Box::new(one()),
            operator: "+".into(),
            right: Box::new(two()),
        }))),
        Node::Expression(Expression::Infix(InfixExpression {
            token: dummy(),
            left: Box::new(two()),
            operator: "+".into(),
            right: Box::new(two()),
        }))
    );

    // prefix
    assert_eq!(
        run_modify(Node::Expression(Expression::Prefix(PrefixExpression {
            token: dummy(),
            operator: "-".into(),
            right: Box::new(one()),
        }))),
        Node::Expression(Expression::Prefix(PrefixExpression {
            token: dummy(),
            operator: "-".into(),
            right: Box::new(two()),
        }))
    );

    // index
    assert_eq!(
        run_modify(Node::Expression(Expression::Index(IndexExpression {
            token: dummy(),
            left: Box::new(one()),
            index: Box::new(one()),
        }))),
        Node::Expression(Expression::Index(IndexExpression {
            token: dummy(),
            left: Box::new(two()),
            index: Box::new(two()),
        }))
    );

    let block_one = BlockStatement {
        token: dummy(),
        statements: vec![Statement::Expression(ExpressionStatement {
            token: dummy(),
            expression: one(),
        })],
    };
    let block_two = BlockStatement {
        token: dummy(),
        statements: vec![Statement::Expression(ExpressionStatement {
            token: dummy(),
            expression: two(),
        })],
    };

    // if/else
    assert_eq!(
        run_modify(Node::Expression(Expression::If(IfExpression {
            token: dummy(),
            condition: Box::new(one()),
            consequence: block_one.clone(),
            alternative: Some(block_one.clone()),
        }))),
        Node::Expression(Expression::If(IfExpression {
            token: dummy(),
            condition: Box::new(two()),
            consequence: block_two.clone(),
            alternative: Some(block_two.clone()),
        }))
    );

    // return
    assert_eq!(
        run_modify(Node::Statement(Statement::Return(ReturnStatement {
            token: dummy(),
            return_value: one(),
        }))),
        Node::Statement(Statement::Return(ReturnStatement {
            token: dummy(),
            return_value: two(),
        }))
    );

    // let
    assert_eq!(
        run_modify(Node::Statement(Statement::Let(LetStatement {
            token: dummy(),
            name: Ident {
                token: dummy(),
                value: String::new()
            },
            value: one(),
        }))),
        Node::Statement(Statement::Let(LetStatement {
            token: dummy(),
            name: Ident {
                token: dummy(),
                value: String::new()
            },
            value: two(),
        }))
    );

    // function literal
    assert_eq!(
        run_modify(Node::Expression(Expression::FunctionLiteral(
            FunctionLiteral {
                token: dummy(),
                parameters: vec![],
                body: block_one.clone(),
            }
        ))),
        Node::Expression(Expression::FunctionLiteral(FunctionLiteral {
            token: dummy(),
            parameters: vec![],
            body: block_two.clone(),
        }))
    );

    // array literal
    assert_eq!(
        run_modify(Node::Expression(Expression::ArrayLiteral(ArrayLiteral {
            token: dummy(),
            elements: vec![one(), one()],
        }))),
        Node::Expression(Expression::ArrayLiteral(ArrayLiteral {
            token: dummy(),
            elements: vec![two(), two()],
        }))
    );

    // hash literal: every key & value 1 -> 2
    let hash = Node::Expression(Expression::HashLiteral(HashLiteral {
        token: dummy(),
        pairs: vec![(one(), one()), (one(), one())],
    }));
    if let Expression::HashLiteral(hl) = run_modify(hash).expect_expression() {
        for (k, v) in hl.pairs {
            match (k, v) {
                (Expression::IntegerLiteral(k), Expression::IntegerLiteral(v)) => {
                    assert_eq!(k.value, 2);
                    assert_eq!(v.value, 2);
                }
                _ => panic!("hash pair not integer literals"),
            }
        }
    } else {
        panic!("not a hash literal");
    }
}

// ---- extra Display / token_literal coverage ----

#[test]
fn display_of_compound_nodes() {
    // Build `if (x) { y } else { z }` by hand and check String().
    let blk = |val: &str| BlockStatement {
        token: Token::new(TokenType::Lbrace, "{"),
        statements: vec![Statement::Expression(ExpressionStatement {
            token: Token::new(TokenType::Ident, val),
            expression: Expression::Ident(Ident {
                token: Token::new(TokenType::Ident, val),
                value: val.into(),
            }),
        })],
    };
    let if_expr = Expression::If(IfExpression {
        token: Token::new(TokenType::If, "if"),
        condition: Box::new(Expression::Ident(Ident {
            token: Token::new(TokenType::Ident, "x"),
            value: "x".into(),
        })),
        consequence: blk("y"),
        alternative: Some(blk("z")),
    });
    // Matches Go's IfExpression.String(): "if" + cond (no space) + " " + conseq + "else " + alt.
    assert_eq!(if_expr.to_string(), "ifx yelse z");

    // index expression
    let idx = Expression::Index(IndexExpression {
        token: Token::new(TokenType::Lbracket, "["),
        left: Box::new(Expression::Ident(Ident {
            token: Token::new(TokenType::Ident, "arr"),
            value: "arr".into(),
        })),
        index: Box::new(int_lit("0")),
    });
    assert_eq!(idx.to_string(), "(arr[0])");
}

fn int_lit(lit: &str) -> Expression {
    Expression::IntegerLiteral(IntegerLiteral {
        token: Token::new(TokenType::Int, lit),
        value: lit.parse().unwrap(),
    })
}

#[test]
fn program_token_literal() {
    let empty = Program { statements: vec![] };
    assert_eq!(empty.token_literal(), "");

    let p = Program {
        statements: vec![Statement::Return(ReturnStatement {
            token: Token::new(TokenType::Return, "return"),
            return_value: int_lit("5"),
        })],
    };
    assert_eq!(p.token_literal(), "return");
}
