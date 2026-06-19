//! Translation of the Go `object` package (`object/object.go` +
//! `object/environment.go`).
//!
//! # Go -> Rust mapping for this module
//!
//! * `Object` interface (`Type()`, `Inspect()`) -> `enum Object`, same as the
//!   AST: a closed set of variants matched exhaustively. Discarded the
//!   `Box<dyn Object>` trait-object translation for the same reasons as in
//!   `ast.rs`.
//! * `Type` (`type Type string`) -> `enum Type` with `as_str()`, mirroring the
//!   token module.
//! * `Hashable` interface -> [`Object::hash_key`] returning `Option<HashKey>`
//!   (`None` == "not hashable", replacing Go's `obj.(Hashable)` assertion).
//! * `Environment` interface + `environment` struct + closures that mutate it ->
//!   `Rc<RefCell<Environment>>` (alias [`Env`]). This is the load-bearing
//!   decision of the whole port: Monkey closures *capture* their defining
//!   environment and may extend it, so the environment must be **shared,
//!   interior-mutable** state. `Rc` gives shared ownership; `RefCell` gives the
//!   runtime-checked mutability Go got for free through pointers + GC.
//!   Discarded: `Box<Environment>` / owned environments (a closure could not
//!   then share its parent scope) and `Arc<Mutex<_>>` (the Go type is
//!   explicitly *not* thread-safe — its own doc comment says so — so the
//!   single-threaded `Rc`/`RefCell` is the precise equivalent).

use crate::ast;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ===========================================================================
// Type
// ===========================================================================

/// Go's `object.Type` (`type Type string`). Strings preserved verbatim because
/// they appear in user-facing error messages ("type mismatch: Integer + ...").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Type {
    Integer,
    Float,
    Boolean,
    Nil,
    ReturnValue,
    Error,
    Function,
    String,
    Builtin,
    Array,
    Hash,
    Quote,
    Macro,
}

impl Type {
    pub fn as_str(&self) -> &'static str {
        match self {
            Type::Integer => "Integer",
            Type::Float => "Float",
            Type::Boolean => "Boolean",
            Type::Nil => "Nil",
            Type::ReturnValue => "ReturnValue",
            Type::Error => "Error",
            Type::Function => "Function",
            Type::String => "String",
            Type::Builtin => "Builtin",
            Type::Array => "Array",
            Type::Hash => "Hash",
            Type::Quote => "Quote",
            Type::Macro => "Macro",
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ===========================================================================
// HashKey / hashing
// ===========================================================================

/// Go's `object.HashKey` struct (`{Type, Value uint64}`). Derives `Eq + Hash`
/// so it can be a real `HashMap` key in `Hash`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HashKey {
    pub type_: Type,
    pub value: u64,
}

/// FNV-1a 64-bit, reproducing Go's `hash/fnv.New64a()`. Implemented by hand (the
/// algorithm is tiny) rather than pulling in a crate, keeping the port
/// dependency-free *and* byte-for-byte identical to Go's hashing of strings and
/// floats.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ===========================================================================
// Object
// ===========================================================================

/// Builtin function pointer. Go: `type BuiltinFunction func(args ...Object)
/// Object`. Rust: a plain `fn(Vec<Object>) -> Object` — all builtins are
/// non-capturing, so a function pointer (which is `Copy`) suffices and avoids
/// boxing.
pub type BuiltinFunction = fn(Vec<Object>) -> Object;

#[derive(Clone, Debug)]
pub struct Builtin {
    pub func: BuiltinFunction,
}

/// Go's `object.Function`. Heavy fields (captured environment + AST) so the
/// whole thing is shared behind `Rc` in the `Object` enum, making `clone()`
/// cheap and matching Go's pointer (`*Function`) semantics.
#[derive(Clone, Debug)]
pub struct Function {
    pub parameters: Vec<ast::Ident>,
    pub body: ast::BlockStatement,
    pub env: Env,
}

/// Go's `object.Macro`.
#[derive(Clone, Debug)]
pub struct Macro {
    pub parameters: Vec<ast::Ident>,
    pub body: ast::BlockStatement,
    pub env: Env,
}

/// Go's `object.HashPair`.
#[derive(Clone, Debug)]
pub struct HashPair {
    pub key: Object,
    pub value: Object,
}

/// Go's `object.Hash` (`Pairs map[HashKey]HashPair`). Kept as a real `HashMap`
/// to preserve O(1) index lookup semantics. As in Go, `Inspect()` iteration
/// order is therefore unspecified.
#[derive(Clone, Debug)]
pub struct HashObj {
    pub pairs: HashMap<HashKey, HashPair>,
}

/// Go's `object.Object` interface, as a sum type.
///
/// Indirection choices:
///   * `ReturnValue(Box<Object>)`, `Quote(Box<ast::Node>)` — single owned child.
///   * `Function`/`Macro`/`Array`/`Hash` wrapped in `Rc` — shared,
///     reference-counted, so cloning an `Object` that moves through `eval` is
///     cheap and mirrors Go pointers.
///   * `Str(String)` — Go names this `String`; renamed to `Str` to avoid
///     shadowing the ubiquitous `std::string::String` inside this file.
#[derive(Clone, Debug)]
pub enum Object {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    ReturnValue(Box<Object>),
    Error(String),
    Function(Rc<Function>),
    Str(String),
    Builtin(Builtin),
    Array(Rc<Vec<Object>>),
    Hash(Rc<HashObj>),
    Quote(Box<ast::Node>),
    Macro(Rc<Macro>),
}

impl Object {
    /// Go's `Type()` method across all object types.
    pub fn type_(&self) -> Type {
        match self {
            Object::Integer(_) => Type::Integer,
            Object::Float(_) => Type::Float,
            Object::Boolean(_) => Type::Boolean,
            Object::Nil => Type::Nil,
            Object::ReturnValue(_) => Type::ReturnValue,
            Object::Error(_) => Type::Error,
            Object::Function(_) => Type::Function,
            Object::Str(_) => Type::String,
            Object::Builtin(_) => Type::Builtin,
            Object::Array(_) => Type::Array,
            Object::Hash(_) => Type::Hash,
            Object::Quote(_) => Type::Quote,
            Object::Macro(_) => Type::Macro,
        }
    }

    /// Go's `Inspect()` method.
    pub fn inspect(&self) -> String {
        match self {
            Object::Integer(v) => v.to_string(),
            // Go: strconv.FormatFloat(v, 'f', -1, 64) — shortest, non-exponential.
            // Rust's f64 `Display` is likewise shortest round-trip and never
            // exponential, so the outputs coincide for all finite values.
            Object::Float(v) => format_float(*v),
            Object::Boolean(v) => v.to_string(),
            Object::Nil => "nil".to_string(),
            Object::ReturnValue(v) => v.inspect(),
            Object::Error(msg) => format!("Error: {}", msg),
            Object::Function(func) => {
                let params: Vec<String> =
                    func.parameters.iter().map(|p| p.to_string()).collect();
                format!("fn({}) {{\n{}\n}}", params.join(", "), func.body)
            }
            Object::Str(v) => v.clone(),
            Object::Builtin(_) => "builtin function".to_string(),
            Object::Array(elements) => {
                let elems: Vec<String> = elements.iter().map(|e| e.inspect()).collect();
                format!("[{}]", elems.join(", "))
            }
            Object::Hash(h) => {
                let pairs: Vec<String> = h
                    .pairs
                    .values()
                    .map(|p| format!("{}: {}", p.key.inspect(), p.value.inspect()))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            // Go: fmt.Sprintf("%s(%s)", QuoteType, node.String()).
            Object::Quote(node) => format!("{}({})", Type::Quote.as_str(), node),
            Object::Macro(m) => {
                let params: Vec<String> = m.parameters.iter().map(|p| p.to_string()).collect();
                format!("macro({}) {{\n{}\n}}", params.join(", "), m.body)
            }
        }
    }

    /// Go's `Hashable` interface, collapsed into a method. Returns `None` for
    /// types that are not hashable (Go would fail the `obj.(Hashable)`
    /// assertion); the caller turns that into the "unusable as hash key" error.
    pub fn hash_key(&self) -> Option<HashKey> {
        match self {
            Object::Integer(v) => Some(HashKey {
                type_: Type::Integer,
                value: *v as u64, // Go: uint64(i.Value)
            }),
            Object::Boolean(v) => Some(HashKey {
                type_: Type::Boolean,
                value: if *v { 1 } else { 0 },
            }),
            Object::Str(v) => Some(HashKey {
                type_: Type::String,
                value: fnv1a_64(v.as_bytes()),
            }),
            // Go's Float also implements Hashable (FNV of its formatted string),
            // even though it is never used as a literal key by the parser.
            Object::Float(v) => Some(HashKey {
                type_: Type::Float,
                value: fnv1a_64(format_float(*v).as_bytes()),
            }),
            _ => None,
        }
    }
}

/// Reproduces `strconv.FormatFloat(v, 'f', -1, 64)`. Factored out because both
/// `Inspect` and the float `HashKey` need the identical string.
fn format_float(v: f64) -> String {
    format!("{}", v)
}

// ===========================================================================
// Environment — translation of object/environment.go
// ===========================================================================

/// Shared, interior-mutable environment handle. See module docs.
pub type Env = Rc<RefCell<Environment>>;

/// Go's unexported `environment` struct.
#[derive(Debug)]
pub struct Environment {
    store: HashMap<String, Object>,
    outer: Option<Env>,
}

impl Environment {
    /// Go's `NewEnvironment`. Returns the shared handle (`Env`) directly, since
    /// every caller stores it in something shareable (a `Function`, a closure).
    pub fn new() -> Env {
        Rc::new(RefCell::new(Environment {
            store: HashMap::new(),
            outer: None,
        }))
    }

    /// Go's `NewEnclosedEnvironment`.
    pub fn new_enclosed(outer: Env) -> Env {
        Rc::new(RefCell::new(Environment {
            store: HashMap::new(),
            outer: Some(outer),
        }))
    }

    /// Go's `Get` (which returned `(Object, bool)`). Rust idiom: `Option`.
    /// Walks the `outer` chain just like Go.
    pub fn get(&self, name: &str) -> Option<Object> {
        match self.store.get(name) {
            Some(obj) => Some(obj.clone()),
            None => match &self.outer {
                Some(outer) => outer.borrow().get(name),
                None => None,
            },
        }
    }

    /// Go's `Set` (returns the value it stored).
    pub fn set(&mut self, name: String, val: Object) -> Object {
        self.store.insert(name, val.clone());
        val
    }
}
