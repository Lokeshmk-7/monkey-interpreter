//! Translation of `eval/quote.go` — the `quote`/`unquote` machinery for macros.
//!
//! # Go -> Rust mapping for this module
//!
//! * The `FuncNameQuote`/`FuncNameUnquote` consts -> `pub(crate) const &str`.
//! * `evalUnquoteCalls` builds a `ModifierFunc` closure and hands it to
//!   `ast.Modify`. Rust: the same, with a `FnMut` closure passed as
//!   `&mut dyn FnMut(Node) -> Node`. The closure captures `env` by reference and
//!   calls `eval`, which is why `FnMut` (not `Fn`) is required.
//! * `convertObjectToASTNode` returns `ast.Node` and may return `nil` for
//!   unsupported objects. Rust returns `Option<Node>`; on `None` the modifier
//!   leaves the node unchanged instead of substituting a null node. This is
//!   strictly safer (no nil-deref) and unreachable for the supported object
//!   kinds (Integer/Boolean/Quote) that tests exercise.

use super::eval;
use crate::ast::{self, Boolean, Expression, IntegerLiteral, Node};
use crate::object::{Env, Object};
use crate::token::{Token, TokenType};

pub(crate) const FUNC_NAME_QUOTE: &str = "quote";
pub(crate) const FUNC_NAME_UNQUOTE: &str = "unquote";

/// Go's `quote(node, env)`.
pub(crate) fn quote(node: Node, env: &Env) -> Object {
    let node = eval_unquote_calls(node, env);
    Object::Quote(Box::new(node))
}

fn eval_unquote_calls(quoted: Node, env: &Env) -> Node {
    let mut modifier = |node: Node| -> Node {
        // Go: `call, ok := node.(*ast.CallExpression); if !ok || ... { return node }`.
        if let Node::Expression(Expression::Call(call)) = &node {
            if call.function.token_literal() == FUNC_NAME_UNQUOTE && call.arguments.len() == 1 {
                let unquoted = eval(&Node::Expression(call.arguments[0].clone()), env);
                if let Some(converted) = convert_object_to_ast_node(unquoted) {
                    return converted;
                }
            }
        }
        node
    };
    ast::modify(quoted, &mut modifier)
}

/// Go's `convertObjectToASTNode`. Returns `None` where Go returns `nil`.
fn convert_object_to_ast_node(obj: Option<Object>) -> Option<Node> {
    match obj {
        Some(Object::Integer(value)) => {
            let token = Token::new(TokenType::Int, value.to_string());
            Some(Node::Expression(Expression::IntegerLiteral(
                IntegerLiteral { token, value },
            )))
        }
        Some(Object::Boolean(value)) => {
            let token = if value {
                Token::new(TokenType::True, "true")
            } else {
                Token::new(TokenType::False, "false")
            };
            Some(Node::Expression(Expression::Boolean(Boolean {
                token,
                value,
            })))
        }
        Some(Object::Quote(node)) => Some(*node),
        _ => None,
    }
}
