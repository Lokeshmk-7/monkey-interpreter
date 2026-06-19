//! Integration tests for the `object` module — ports `object/object_test.go`
//! (hash keys) plus new coverage of `Type`, `inspect`, and `Environment`.

use monkey::ast::{IntegerLiteral, Node, Expression};
use monkey::object::{Environment, HashObj, HashPair, Object, Type};
use monkey::token::{Token, TokenType};
use std::collections::HashMap;
use std::rc::Rc;

// ---- ported hash-key tests ----

#[test]
fn string_hash_key() {
    let hello1 = Object::Str("Hello World".into());
    let hello2 = Object::Str("Hello World".into());
    let diff1 = Object::Str("My name is johnny".into());
    let diff2 = Object::Str("My name is johnny".into());

    assert_eq!(hello1.hash_key(), hello2.hash_key());
    assert_eq!(diff1.hash_key(), diff2.hash_key());
    assert_ne!(hello1.hash_key(), diff1.hash_key());
}

#[test]
fn boolean_hash_key() {
    assert_eq!(
        Object::Boolean(true).hash_key(),
        Object::Boolean(true).hash_key()
    );
    assert_eq!(
        Object::Boolean(false).hash_key(),
        Object::Boolean(false).hash_key()
    );
    assert_ne!(
        Object::Boolean(true).hash_key(),
        Object::Boolean(false).hash_key()
    );
}

#[test]
fn integer_hash_key() {
    assert_eq!(Object::Integer(1).hash_key(), Object::Integer(1).hash_key());
    assert_eq!(Object::Integer(2).hash_key(), Object::Integer(2).hash_key());
    assert_ne!(Object::Integer(1).hash_key(), Object::Integer(2).hash_key());
}

#[test]
fn non_hashable_objects_have_no_key() {
    assert!(Object::Nil.hash_key().is_none());
    assert!(Object::Array(Rc::new(vec![])).hash_key().is_none());
    assert!(Object::Error("x".into()).hash_key().is_none());
}

#[test]
fn float_hash_key_is_consistent() {
    assert_eq!(Object::Float(1.5).hash_key(), Object::Float(1.5).hash_key());
    assert_ne!(Object::Float(1.5).hash_key(), Object::Float(2.5).hash_key());
}

// ---- Type ----

#[test]
fn type_strings() {
    let cases: &[(Type, &str)] = &[
        (Type::Integer, "Integer"),
        (Type::Float, "Float"),
        (Type::Boolean, "Boolean"),
        (Type::Nil, "Nil"),
        (Type::ReturnValue, "ReturnValue"),
        (Type::Error, "Error"),
        (Type::Function, "Function"),
        (Type::String, "String"),
        (Type::Builtin, "Builtin"),
        (Type::Array, "Array"),
        (Type::Hash, "Hash"),
        (Type::Quote, "Quote"),
        (Type::Macro, "Macro"),
    ];
    for (ty, want) in cases {
        assert_eq!(ty.as_str(), *want);
        assert_eq!(format!("{}", ty), *want);
    }
}

#[test]
fn object_type_tags() {
    assert_eq!(Object::Integer(1).type_(), Type::Integer);
    assert_eq!(Object::Float(1.0).type_(), Type::Float);
    assert_eq!(Object::Boolean(true).type_(), Type::Boolean);
    assert_eq!(Object::Nil.type_(), Type::Nil);
    assert_eq!(Object::Str("x".into()).type_(), Type::String);
    assert_eq!(Object::Error("e".into()).type_(), Type::Error);
    assert_eq!(
        Object::ReturnValue(Box::new(Object::Integer(1))).type_(),
        Type::ReturnValue
    );
    assert_eq!(Object::Array(Rc::new(vec![])).type_(), Type::Array);
}

// ---- inspect ----

#[test]
fn inspect_scalars() {
    assert_eq!(Object::Integer(5).inspect(), "5");
    assert_eq!(Object::Integer(-42).inspect(), "-42");
    assert_eq!(Object::Float(12.34).inspect(), "12.34");
    assert_eq!(Object::Float(78.0).inspect(), "78"); // shortest, non-exponential
    assert_eq!(Object::Boolean(true).inspect(), "true");
    assert_eq!(Object::Boolean(false).inspect(), "false");
    assert_eq!(Object::Nil.inspect(), "nil");
    assert_eq!(Object::Str("hi".into()).inspect(), "hi");
    assert_eq!(Object::Error("boom".into()).inspect(), "Error: boom");
    assert_eq!(
        Object::ReturnValue(Box::new(Object::Integer(9))).inspect(),
        "9"
    );
}

#[test]
fn inspect_array() {
    let arr = Object::Array(Rc::new(vec![
        Object::Integer(1),
        Object::Str("two".into()),
        Object::Boolean(true),
    ]));
    assert_eq!(arr.inspect(), "[1, two, true]");
    assert_eq!(Object::Array(Rc::new(vec![])).inspect(), "[]");
}

#[test]
fn inspect_hash_single_entry() {
    let key = Object::Str("one".into());
    let mut pairs = HashMap::new();
    pairs.insert(
        key.hash_key().unwrap(),
        HashPair {
            key,
            value: Object::Integer(1),
        },
    );
    let hash = Object::Hash(Rc::new(HashObj { pairs }));
    assert_eq!(hash.inspect(), "{one: 1}");
}

#[test]
fn inspect_quote() {
    let node = Node::Expression(Expression::IntegerLiteral(IntegerLiteral {
        token: Token::new(TokenType::Int, "5"),
        value: 5,
    }));
    let q = Object::Quote(Box::new(node));
    assert_eq!(q.inspect(), "Quote(5)");
}

// ---- Environment ----

#[test]
fn environment_get_set() {
    let env = Environment::new();
    let returned = env.borrow_mut().set("x".into(), Object::Integer(5));
    assert!(matches!(returned, Object::Integer(5)));
    assert!(matches!(env.borrow().get("x"), Some(Object::Integer(5))));
    assert!(env.borrow().get("missing").is_none());
}

#[test]
fn enclosed_environment_resolution() {
    let outer = Environment::new();
    outer.borrow_mut().set("x".into(), Object::Integer(5));
    outer.borrow_mut().set("y".into(), Object::Integer(7));

    let inner = Environment::new_enclosed(outer.clone());
    // shadow x, leave y to fall through to outer
    inner.borrow_mut().set("x".into(), Object::Integer(99));

    assert!(matches!(inner.borrow().get("x"), Some(Object::Integer(99))));
    assert!(matches!(inner.borrow().get("y"), Some(Object::Integer(7))));
    // outer is unaffected by the inner shadow
    assert!(matches!(outer.borrow().get("x"), Some(Object::Integer(5))));
    assert!(inner.borrow().get("z").is_none());
}
