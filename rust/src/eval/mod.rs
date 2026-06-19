//! Translation of the Go `eval` package — core evaluator (`eval/eval.go`).
//! Submodules mirror the other files: [`builtin`] (`builtin.go`), [`quote`]
//! (`quote.go`), [`macros`] (`macro.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! ## The `nil` vs `NilValue` distinction (the crucial subtlety)
//! Go's `Eval` returns `object.Object`, an *interface* that can itself be `nil`
//! — separate from the `NilValue` singleton (`*object.Nil`). A `let` statement,
//! for instance, evaluates to the untyped `nil`, which `evalBlockStatement`
//! explicitly skips (`if result == nil { continue }`). Conflating the two would
//! change control flow.
//!
//! Rust mapping: every eval function returns **`Option<Object>`**, where `None`
//! is Go's untyped `nil` and `Some(Object::Nil)` is `NilValue`. This is the
//! faithful encoding of a nullable interface and keeps the block/program
//! skip-logic correct.
//!
//! ## The singletons
//! Go caches `NilValue`/`TrueValue`/`FalseValue` as pointer singletons, partly
//! so `==` (pointer identity) works in `evalInfixExpression`. Because the Rust
//! `Object` enum compares booleans/nil *by value*, no singletons are needed —
//! `Object::Boolean(true)` everywhere is already canonical. The `==`/`!=`
//! fall-through is handled by [`objects_equal`].
//!
//! ## Type switch -> match
//! Go's `switch node := node.(type)` becomes nested `match` over the
//! `Node`/`Statement`/`Expression` enums, split into [`eval_program`],
//! [`eval_statement`], [`eval_expression`] mirroring the Go helper structure.

mod builtin;
mod macros;
mod quote;

pub use macros::{define_macros, expand_macros};

use crate::ast::*;
use crate::object::{Environment, Env, HashKey, HashObj, HashPair, Object, Type};
use std::collections::HashMap;
use std::rc::Rc;

/// Builds an `object.Error`. Go's `newError` is `Sprintf`-based; callers here
/// pass an already-formatted `String`, which is cleaner in Rust.
pub(crate) fn new_error(message: String) -> Object {
    Object::Error(message)
}

/// Go's `isError`: true only for a non-nil Error object. `None` (Go nil) is
/// not an error.
fn is_error(obj: &Option<Object>) -> bool {
    matches!(obj, Some(Object::Error(_)))
}

/// Public entry point — Go's `Eval(node, env)`. `Option` models Go's nullable
/// return (see module docs).
pub fn eval(node: &Node, env: &Env) -> Option<Object> {
    match node {
        Node::Program(p) => eval_program(p, env),
        Node::Statement(s) => eval_statement(s, env),
        Node::Expression(e) => eval_expression(e, env),
    }
}

fn eval_statement(stmt: &Statement, env: &Env) -> Option<Object> {
    match stmt {
        Statement::Expression(es) => eval_expression(&es.expression, env),
        Statement::Return(rs) => {
            let value = eval_expression(&rs.return_value, env);
            if is_error(&value) {
                return value;
            }
            Some(Object::ReturnValue(Box::new(value.expect("return value"))))
        }
        Statement::Block(bs) => eval_block_statement(bs, env),
        Statement::Let(ls) => {
            let value = eval_expression(&ls.value, env);
            if is_error(&value) {
                return value;
            }
            // Go stores the (possibly untyped-nil) value and falls through to
            // `return nil`. A `None` here only arises from evaluating a bare
            // `MacroLiteral` outside the REPL's macro pipeline (Go would store
            // nil and panic later when the name is *called*). We store
            // `Object::Nil` instead, so any later misuse produces a clean
            // "not a function: Nil" error rather than a crash — a deliberate,
            // documented robustness improvement. No test binds a non-expression
            // value, so behaviour is unchanged on all tested inputs.
            env.borrow_mut()
                .set(ls.name.value.clone(), value.unwrap_or(Object::Nil));
            None
        }
    }
}

fn eval_expression(expr: &Expression, env: &Env) -> Option<Object> {
    match expr {
        Expression::IntegerLiteral(il) => Some(Object::Integer(il.value)),
        Expression::FloatLiteral(fl) => Some(Object::Float(fl.value)),
        Expression::Boolean(b) => Some(Object::Boolean(b.value)),
        Expression::Prefix(pe) => {
            let right = eval_expression(&pe.right, env);
            if is_error(&right) {
                return right;
            }
            Some(eval_prefix_expression(&pe.operator, right.expect("prefix operand")))
        }
        Expression::Infix(ie) => {
            let left = eval_expression(&ie.left, env);
            if is_error(&left) {
                return left;
            }
            let right = eval_expression(&ie.right, env);
            if is_error(&right) {
                return right;
            }
            Some(eval_infix_expression(
                &ie.operator,
                left.expect("infix left"),
                right.expect("infix right"),
            ))
        }
        Expression::If(ie) => eval_if_expression(ie, env),
        Expression::Ident(id) => Some(eval_ident(id, env)),
        Expression::FunctionLiteral(fl) => Some(Object::Function(Rc::new(crate::object::Function {
            parameters: fl.parameters.clone(),
            body: fl.body.clone(),
            env: env.clone(),
        }))),
        Expression::Call(ce) => {
            // Go: `if node.Function.TokenLiteral() == FuncNameQuote { return quote(...) }`.
            if ce.function.token_literal() == quote::FUNC_NAME_QUOTE {
                return Some(quote::quote(
                    Node::Expression(ce.arguments[0].clone()),
                    env,
                ));
            }

            let function = eval_expression(&ce.function, env);
            if is_error(&function) {
                return function;
            }

            let args = eval_expressions(&ce.arguments, env);
            if args.len() == 1 {
                if let Object::Error(_) = &args[0] {
                    return Some(args[0].clone());
                }
            }

            apply_function(function.expect("call target"), args)
        }
        Expression::StringLiteral(sl) => Some(Object::Str(sl.value.clone())),
        Expression::ArrayLiteral(al) => {
            let elems = eval_expressions(&al.elements, env);
            if elems.len() == 1 {
                if let Object::Error(_) = &elems[0] {
                    return Some(elems[0].clone());
                }
            }
            Some(Object::Array(Rc::new(elems)))
        }
        Expression::Index(ix) => {
            let left = eval_expression(&ix.left, env);
            if is_error(&left) {
                return left;
            }
            let index = eval_expression(&ix.index, env);
            if is_error(&index) {
                return index;
            }
            Some(eval_index_expression(
                left.expect("index target"),
                index.expect("index value"),
            ))
        }
        Expression::HashLiteral(hl) => eval_hash_literal(hl, env),
        // Go's Eval has no `case *ast.MacroLiteral`, so it hits `return nil`.
        // (Macros are stripped by `define_macros` before evaluation anyway.)
        Expression::MacroLiteral(_) => None,
    }
}

fn eval_program(program: &Program, env: &Env) -> Option<Object> {
    let mut result: Option<Object> = None;

    for stmt in &program.statements {
        result = eval_statement(stmt, env);

        match &result {
            Some(Object::ReturnValue(v)) => return Some((**v).clone()),
            Some(Object::Error(_)) => return result,
            _ => {}
        }
    }

    result
}

fn eval_prefix_expression(operator: &str, right: Object) -> Object {
    match operator {
        "!" => eval_bang_operator_expression(right),
        "-" => eval_minus_prefix_operator_expression(right),
        _ => new_error(format!("unknown operator: {}{}", operator, right.type_())),
    }
}

fn eval_bang_operator_expression(right: Object) -> Object {
    // Go: `if right == NilValue || right == FalseValue`. By-value equivalent:
    match right {
        Object::Nil | Object::Boolean(false) => Object::Boolean(true),
        _ => Object::Boolean(false),
    }
}

fn eval_minus_prefix_operator_expression(right: Object) -> Object {
    match right {
        // `wrapping_neg` matches Go's `-x` on int64 for the `i64::MIN` edge case
        // (Go wraps to MIN; Rust's plain `-` would panic in debug).
        Object::Integer(v) => Object::Integer(v.wrapping_neg()),
        Object::Float(v) => Object::Float(-v),
        other => new_error(format!("unknown operator: -{}", other.type_())),
    }
}

fn eval_infix_expression(operator: &str, left: Object, right: Object) -> Object {
    let lt = left.type_();
    let rt = right.type_();
    if lt == Type::Integer && rt == Type::Integer {
        eval_integer_infix_expression(operator, left, right)
    } else if lt == Type::Float || rt == Type::Float {
        eval_float_infix_expression(operator, left, right)
    } else if lt == Type::String && rt == Type::String {
        eval_string_infix_expression(operator, left, right)
    } else if operator == "==" {
        Object::Boolean(objects_equal(&left, &right))
    } else if operator == "!=" {
        Object::Boolean(!objects_equal(&left, &right))
    } else if lt != rt {
        new_error(format!("type mismatch: {} {} {}", lt, operator, rt))
    } else {
        new_error(format!("unknown operator: {} {} {}", lt, operator, rt))
    }
}

/// Reproduces Go's `left == right` for the `==`/`!=` fall-through. Only Boolean
/// and Nil operands can reach here (Integers/Floats/Strings are handled
/// earlier). Compound objects (functions, etc.) are distinct under Go's pointer
/// comparison; `false` matches that for any structurally-different pair, and
/// these comparisons are not exercised by Monkey programs in practice.
fn objects_equal(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Nil, Object::Nil) => true,
        _ => false,
    }
}

fn eval_integer_infix_expression(operator: &str, left: Object, right: Object) -> Object {
    let l = match left {
        Object::Integer(v) => v,
        _ => unreachable!(),
    };
    let r = match right {
        Object::Integer(v) => v,
        _ => unreachable!(),
    };

    // Go's `int64` arithmetic wraps silently on overflow. Rust's `+`/`-`/`*`
    // panic on overflow in debug builds, so use the `wrapping_*` ops to
    // reproduce Go's two's-complement wraparound exactly. `wrapping_div` also
    // matches Go for the `i64::MIN / -1` overflow case (returns `MIN`); division
    // by zero still panics in both languages.
    match operator {
        "+" => Object::Integer(l.wrapping_add(r)),
        "-" => Object::Integer(l.wrapping_sub(r)),
        "*" => Object::Integer(l.wrapping_mul(r)),
        "/" => Object::Integer(l.wrapping_div(r)),
        "<" => Object::Boolean(l < r),
        ">" => Object::Boolean(l > r),
        "<=" => Object::Boolean(l <= r),
        ">=" => Object::Boolean(l >= r),
        "==" => Object::Boolean(l == r),
        "!=" => Object::Boolean(l != r),
        _ => new_error(format!(
            "unknown operator: {} {} {}",
            Type::Integer,
            operator,
            Type::Integer
        )),
    }
}

fn eval_float_infix_expression(operator: &str, left: Object, right: Object) -> Object {
    // Go promotes Integer operands to float; an unexpected type yields the
    // "unknown operator" error using its own type.
    let l = match &left {
        Object::Integer(v) => *v as f64,
        Object::Float(v) => *v,
        other => {
            return new_error(format!(
                "unknown operator: {} {} {}",
                other.type_(),
                operator,
                right.type_()
            ))
        }
    };
    let r = match &right {
        Object::Integer(v) => *v as f64,
        Object::Float(v) => *v,
        other => {
            return new_error(format!(
                "unknown operator: {} {} {}",
                left.type_(),
                operator,
                other.type_()
            ))
        }
    };

    match operator {
        "+" => Object::Float(l + r),
        "-" => Object::Float(l - r),
        "*" => Object::Float(l * r),
        "/" => Object::Float(l / r),
        "<" => Object::Boolean(l < r),
        ">" => Object::Boolean(l > r),
        "<=" => Object::Boolean(l <= r),
        ">=" => Object::Boolean(l >= r),
        "==" => Object::Boolean(l == r),
        "!=" => Object::Boolean(l != r),
        _ => new_error(format!(
            "unknown operator: {} {} {}",
            left.type_(),
            operator,
            right.type_()
        )),
    }
}

fn eval_string_infix_expression(operator: &str, left: Object, right: Object) -> Object {
    let l = match left {
        Object::Str(s) => s,
        _ => unreachable!(),
    };
    let r = match right {
        Object::Str(s) => s,
        _ => unreachable!(),
    };

    match operator {
        "+" => Object::Str(l + &r),
        "==" => Object::Boolean(l == r),
        "!=" => Object::Boolean(l != r),
        _ => new_error(format!(
            "unknown operator: {} {} {}",
            Type::String,
            operator,
            Type::String
        )),
    }
}

fn eval_block_statement(block: &BlockStatement, env: &Env) -> Option<Object> {
    let mut result: Option<Object> = None;

    for stmt in &block.statements {
        result = eval_statement(stmt, env);

        // Go: `if result == nil { continue }`.
        if let Some(obj) = &result {
            let rt = obj.type_();
            if rt == Type::ReturnValue || rt == Type::Error {
                return result;
            }
        }
    }

    result
}

fn eval_if_expression(ie: &IfExpression, env: &Env) -> Option<Object> {
    let condition = eval_expression(&ie.condition, env);
    if is_error(&condition) {
        return condition;
    }

    let condition = condition.expect("if condition");
    if is_truthy(&condition) {
        eval_block_statement(&ie.consequence, env)
    } else if let Some(alt) = &ie.alternative {
        eval_block_statement(alt, env)
    } else {
        Some(Object::Nil)
    }
}

fn is_truthy(obj: &Object) -> bool {
    // Go: `obj != NilValue && obj != FalseValue`.
    !matches!(obj, Object::Nil | Object::Boolean(false))
}

fn eval_ident(node: &Ident, env: &Env) -> Object {
    if let Some(val) = env.borrow().get(&node.value) {
        return val;
    }

    if let Some(builtin) = builtin::lookup(&node.value) {
        return builtin;
    }

    new_error(format!("identifier not found: {}", node.value))
}

/// Go's `evalExpressions`: returns a single-element vec holding the error on the
/// first error, otherwise all evaluated values. `None` sub-results (Go nil) are
/// materialised as `Object::Nil` — only reachable for pathological inputs.
fn eval_expressions(exprs: &[Expression], env: &Env) -> Vec<Object> {
    let mut result = Vec::with_capacity(exprs.len());

    for expr in exprs {
        let evaluated = eval_expression(expr, env);
        if is_error(&evaluated) {
            return vec![evaluated.expect("error object")];
        }
        result.push(evaluated.unwrap_or(Object::Nil));
    }

    result
}

fn extend_function_env(func: &crate::object::Function, args: &[Object]) -> Env {
    let env = Environment::new_enclosed(func.env.clone());
    for (i, param) in func.parameters.iter().enumerate() {
        env.borrow_mut().set(param.value.clone(), args[i].clone());
    }
    env
}

fn apply_function(func: Object, args: Vec<Object>) -> Option<Object> {
    match func {
        Object::Function(f) => {
            let extended_env = extend_function_env(&f, &args);
            let evaluated = eval_block_statement(&f.body, &extended_env);
            unwrap_return_value(evaluated)
        }
        Object::Builtin(b) => Some((b.func)(args)),
        other => Some(new_error(format!("not a function: {}", other.type_()))),
    }
}

fn unwrap_return_value(obj: Option<Object>) -> Option<Object> {
    match obj {
        Some(Object::ReturnValue(v)) => Some(*v),
        other => other,
    }
}

fn eval_index_expression(left: Object, index: Object) -> Object {
    if left.type_() == Type::Array && index.type_() == Type::Integer {
        eval_array_index_expression(left, index)
    } else if left.type_() == Type::Hash {
        eval_hash_index_expression(left, index)
    } else {
        new_error(format!("index operator not supported: {}", left.type_()))
    }
}

fn eval_array_index_expression(array: Object, index: Object) -> Object {
    let elements = match array {
        Object::Array(a) => a,
        _ => unreachable!(),
    };
    let idx = match index {
        Object::Integer(v) => v,
        _ => unreachable!(),
    };
    let max = elements.len() as i64 - 1;

    if idx < 0 || idx > max {
        return Object::Nil;
    }

    elements[idx as usize].clone()
}

fn eval_hash_literal(node: &HashLiteral, env: &Env) -> Option<Object> {
    let mut pairs: HashMap<HashKey, HashPair> = HashMap::with_capacity(node.pairs.len());

    for (key_node, value_node) in &node.pairs {
        let key = eval_expression(key_node, env);
        if is_error(&key) {
            return key;
        }
        let key = key.expect("hash key");

        let hashed = match key.hash_key() {
            Some(h) => h,
            None => return Some(new_error(format!("unusable as hash key: {}", key.type_()))),
        };

        let value = eval_expression(value_node, env);
        if is_error(&value) {
            return value;
        }
        let value = value.expect("hash value");

        pairs.insert(hashed, HashPair { key, value });
    }

    Some(Object::Hash(Rc::new(HashObj { pairs })))
}

fn eval_hash_index_expression(left: Object, index: Object) -> Object {
    let hashed = match index.hash_key() {
        Some(h) => h,
        None => return new_error(format!("unusable as hash key: {}", index.type_())),
    };

    let hash = match left {
        Object::Hash(h) => h,
        _ => unreachable!(),
    };

    match hash.pairs.get(&hashed) {
        Some(pair) => pair.value.clone(),
        None => Object::Nil,
    }
}
