//! Translation of `eval/macro.go` — macro definition + expansion.
//!
//! (Renamed from `macro` to `macros` because `macro` is a reserved keyword in
//! Rust.)
//!
//! # Go -> Rust mapping for this module
//!
//! * `DefineMacros` removes macro-defining `let` statements from the program
//!   after registering them. Go records the indices and splices them out in
//!   reverse; Rust does the same with `Vec::remove` in reverse order (removing
//!   high indices first keeps the lower indices valid).
//! * `ExpandMacros` walks the AST with `ast.Modify`, replacing macro *calls*
//!   with the AST the macro body quotes. Same `FnMut` closure approach as
//!   `quote.rs`.
//! * `isMacroCall` returns `(*Macro, bool)`; Rust returns `Option<Rc<Macro>>`.

use super::eval;
use crate::ast::{Expression, Node, Program, Statement};
use crate::object::{Env, Environment, Macro, Object};
use std::rc::Rc;

/// Go's `DefineMacros`. Mutates the program in place (Go takes `*ast.Program`).
pub fn define_macros(program: &mut Program, env: &Env) {
    let mut defs: Vec<usize> = Vec::new();

    for (pos, stmt) in program.statements.iter().enumerate() {
        if is_macro_definition(stmt) {
            add_macro(stmt, env);
            defs.push(pos);
        }
    }

    // Remove from the back so earlier indices stay valid (mirrors Go's reverse
    // splice loop).
    for &pos in defs.iter().rev() {
        program.statements.remove(pos);
    }
}

fn is_macro_definition(node: &Statement) -> bool {
    matches!(
        node,
        Statement::Let(ls) if matches!(ls.value, Expression::MacroLiteral(_))
    )
}

fn add_macro(stmt: &Statement, env: &Env) {
    let ls = match stmt {
        Statement::Let(ls) => ls,
        _ => unreachable!("add_macro called on non-let statement"),
    };
    let macro_lit = match &ls.value {
        Expression::MacroLiteral(m) => m,
        _ => unreachable!("add_macro called on non-macro let"),
    };

    let macro_obj = Object::Macro(Rc::new(Macro {
        parameters: macro_lit.parameters.clone(),
        body: macro_lit.body.clone(),
        env: env.clone(),
    }));
    env.borrow_mut().set(ls.name.value.clone(), macro_obj);
}

/// Go's `ExpandMacros`. Takes the program as a `Node` and returns the rewritten
/// `Node`.
pub fn expand_macros(program: Node, env: &Env) -> Node {
    let mut modifier = |node: Node| -> Node {
        if let Node::Expression(Expression::Call(call)) = &node {
            if let Some(macro_obj) = is_macro_call(call, env) {
                let args = quote_args(call);
                let eval_env = extend_macro_env(&macro_obj, args);

                let evaluated = eval(&Node::Statement(Statement::Block(macro_obj.body.clone())), &eval_env);
                match evaluated {
                    Some(Object::Quote(n)) => return *n,
                    _ => panic!("we only support returning AST-nodes from macros"),
                }
            }
        }
        node
    };

    crate::ast::modify(program, &mut modifier)
}

fn is_macro_call(call: &crate::ast::CallExpression, env: &Env) -> Option<Rc<Macro>> {
    let ident = match &*call.function {
        Expression::Ident(id) => id,
        _ => return None,
    };

    match env.borrow().get(&ident.value) {
        Some(Object::Macro(m)) => Some(m),
        _ => None,
    }
}

/// Go's `quoteArgs`: wraps each unevaluated argument in a `Quote` object.
fn quote_args(call: &crate::ast::CallExpression) -> Vec<Object> {
    call.arguments
        .iter()
        .map(|arg| Object::Quote(Box::new(Node::Expression(arg.clone()))))
        .collect()
}

/// Go's `extendMacroEnv`.
fn extend_macro_env(macro_obj: &Macro, args: Vec<Object>) -> Env {
    let extended = Environment::new_enclosed(macro_obj.env.clone());
    for (i, param) in macro_obj.parameters.iter().enumerate() {
        extended.borrow_mut().set(param.value.clone(), args[i].clone());
    }
    extended
}
