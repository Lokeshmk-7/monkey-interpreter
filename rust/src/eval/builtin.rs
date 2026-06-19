//! Translation of `eval/builtin.go`.
//!
//! # Go -> Rust mapping for this module
//!
//! Go stores builtins in `var builtins = map[string]*object.Builtin{...}`, each
//! value wrapping an anonymous closure. Rust mapping: top-level `fn`s plus a
//! [`lookup`] that returns the matching `Object::Builtin` (its `func` is a plain
//! function pointer). A `match`-based lookup avoids a lazily-initialised global
//! map and keeps the builtins as ordinary, individually-testable functions.
//!
//! Faithful quirk preserved: Go's `rest` builtin reports its error as
//! "argument to `last` must be Array" (a copy-paste bug in the original). The
//! port keeps the identical message for I/O equivalence.

use super::new_error;
use crate::object::{Builtin, BuiltinFunction, Object};
use std::rc::Rc;

/// Go's `builtins` map lookup. Returns `None` when the name is not a builtin
/// (the caller then produces "identifier not found").
pub(crate) fn lookup(name: &str) -> Option<Object> {
    let func: BuiltinFunction = match name {
        "len" => builtin_len,
        "first" => builtin_first,
        "last" => builtin_last,
        "rest" => builtin_rest,
        "push" => builtin_push,
        "puts" => builtin_puts,
        _ => return None,
    };
    Some(Object::Builtin(Builtin { func }))
}

fn builtin_len(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return new_error(format!(
            "wrong number of arguments. want=1, got={}",
            args.len()
        ));
    }

    match &args[0] {
        // Go uses byte length for strings (`len` of a Go string); Rust's
        // `str::len` is also byte length, so values match.
        Object::Str(s) => Object::Integer(s.len() as i64),
        Object::Array(elements) => Object::Integer(elements.len() as i64),
        other => new_error(format!(
            "argument to `len` not supported, got {}",
            other.type_()
        )),
    }
}

fn builtin_first(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return new_error(format!(
            "wrong number of arguments. want=1, got={}",
            args.len()
        ));
    }

    let elements = match &args[0] {
        Object::Array(a) => a,
        other => {
            return new_error(format!(
                "argument to `first` must be Array, got {}",
                other.type_()
            ))
        }
    };

    match elements.first() {
        Some(obj) => obj.clone(),
        None => Object::Nil,
    }
}

fn builtin_last(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return new_error(format!(
            "wrong number of arguments. want=1, got={}",
            args.len()
        ));
    }

    let elements = match &args[0] {
        Object::Array(a) => a,
        other => {
            return new_error(format!(
                "argument to `last` must be Array, got {}",
                other.type_()
            ))
        }
    };

    match elements.last() {
        Some(obj) => obj.clone(),
        None => Object::Nil,
    }
}

fn builtin_rest(args: Vec<Object>) -> Object {
    if args.len() != 1 {
        return new_error(format!(
            "wrong number of arguments. want=1, got={}",
            args.len()
        ));
    }

    let elements = match &args[0] {
        Object::Array(a) => a,
        other => {
            // NOTE: message says "last", matching the Go source's copy-paste bug.
            return new_error(format!(
                "argument to `last` must be Array, got {}",
                other.type_()
            ))
        }
    };

    if elements.is_empty() {
        return Object::Nil;
    }

    let new_elems: Vec<Object> = elements[1..].to_vec();
    Object::Array(Rc::new(new_elems))
}

fn builtin_push(args: Vec<Object>) -> Object {
    if args.len() != 2 {
        return new_error(format!(
            "wrong number of arguments. want=2, got={}",
            args.len()
        ));
    }

    let elements = match &args[0] {
        Object::Array(a) => a,
        other => {
            return new_error(format!(
                "first argument to `push` must be Array, got {}",
                other.type_()
            ))
        }
    };

    let mut new_elems = Vec::with_capacity(elements.len() + 1);
    new_elems.extend(elements.iter().cloned());
    new_elems.push(args[1].clone());
    Object::Array(Rc::new(new_elems))
}

fn builtin_puts(args: Vec<Object>) -> Object {
    for arg in &args {
        println!("{}", arg.inspect());
    }
    Object::Nil
}
