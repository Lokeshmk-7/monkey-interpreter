# Construct Mappability — Detailed, Worked Examples (Go → Rust)

This document is the evidence base behind the **Construct Mappability Score
(CMS)** reported in the experiment. It classifies every Go construct category in
the Monkey interpreter into one of three classes and shows the *actual* code for
representative examples, with the rationale and the discarded alternative.

## Classification rubric

| Class | Definition | Design decision required? | Semantics |
|-------|------------|---------------------------|-----------|
| **Direct** | A 1:1 counterpart exists; only trivial syntactic change. | No | Identical |
| **Adapted (idiomatic adaptation)** | The target language has an idiomatic equivalent of the *same concept*, but the form changes and a deliberate choice is made. | Yes | Preserved, mechanism re-expressed |
| **Re-architected** | No direct equivalent; the underlying *mechanism/structure* is replaced. | Yes (structural) | Preserved by a different design |

Counts for this object: **Direct = 12, Adapted = 9, Re-architected = 1,
Unmappable = 0** (22 categories).
`CMS = (12·1.0 + 9·0.5 + 1·0.25)/22 = 0.76`; direct-only = 54.5 %; mappable = 100 %.

---

## 1. Direct mappings (12)

These needed no design decision — the LLM emitted them mechanically and they
compiled/behaved identically. Representative examples:

### 1.1 struct → struct
```go
// Go: token/token.go
type Token struct {
    Type    Type
    Literal string
}
```
```rust
// Rust: src/token.rs   (only deviation: field `Type` -> `token_type`, a keyword clash)
pub struct Token {
    pub token_type: TokenType,
    pub literal: String,
}
```

### 1.2 slice `[]T` → `Vec<T>`
```go
type Program struct { Statements []Statement }      // Go
```
```rust
pub struct Program { pub statements: Vec<Statement> } // Rust
```

### 1.3 `map[K]V` (lookup-keyed) → `HashMap<K,V>`
```go
type Hash struct { Pairs map[HashKey]HashPair }       // object/object.go
```
```rust
pub struct HashObj { pub pairs: HashMap<HashKey, HashPair> }  // src/object.rs
```
Kept as a real hash map because the only operation is O(1) keyed lookup
(`evalHashIndexExpression`); iteration order is unspecified in both languages.

Other Direct categories: `func`→`fn`, method→inherent `impl` method,
`fmt.Sprintf`→`format!`, `strconv`↔`parse`/`to_string`,
`bytes.Buffer`/`strings.Join`→`String`/`join`, closures→closures,
`(v, ok)` tuple destructuring→tuple, `io` traits→`io` traits.

---

## 2. Idiomatic adaptations (9)

Each preserves semantics but changes the mechanism. For every one: Go form,
Rust form, *why*, and the discarded alternative.

### 2.1 Interface + type switch → `enum` + `match`  ★ load-bearing
```go
// Go: a closed family expressed as an interface + "marker method", recovered
// with a type switch.
type Expression interface { Node; expressionNode() }

switch node := node.(type) {       // ast/modify.go, eval/eval.go
case *ast.IntegerLiteral: ...
case *ast.InfixExpression: ...
}
```
```rust
// Rust: an algebraic data type; the type switch becomes an exhaustive match.
pub enum Expression {
    IntegerLiteral(IntegerLiteral),
    Infix(InfixExpression),
    /* ...12 more... */
}

match expr {
    Expression::IntegerLiteral(il) => Some(Object::Integer(il.value)),
    Expression::Infix(ie) => { /* ... */ }
    /* compiler errors if a variant is forgotten */
}
```
**Why adaptation, not direct:** Go interfaces are open (any type can implement
them) and dispatch is dynamic; the enum is closed and dispatch is a `match` the
compiler checks for exhaustiveness. This turns Go's runtime "marker method"
convention into a compile-time guarantee.
**Discarded alternative:** `Box<dyn Node>` trait objects + `Any` downcasting —
the *literal* translation, but it reintroduces the very runtime type errors the
enum removes and needs `as_any()` boilerplate on every node.

### 2.2 Nil-able interface return → `Option<T>`  ★ the subtle one
```go
// Go: Eval returns object.Object, which can itself be a nil interface,
// distinct from the NilValue singleton. evalBlockStatement relies on it:
result = Eval(stmt, env)
if result == nil { continue }            // skip e.g. a `let`
```
```rust
// Rust: None == Go's untyped nil; Some(Object::Nil) == NilValue.
fn eval(node: &Node, env: &Env) -> Option<Object> { /* ... */ }

if let Some(obj) = &result {              // None (Go nil) is skipped
    let rt = obj.type_();
    if rt == Type::ReturnValue || rt == Type::Error { return result; }
}
```
**Why:** Rust has no "null reference"; the two distinct Go nullities (`nil` vs
`NilValue`) map cleanly onto `Option<Object>` vs `Object::Nil`. Conflating them
would change block/program control flow.
**Discarded alternative:** a sentinel `Object::Untyped` variant — pollutes the
value model with a non-value.

### 2.3 Pratt dispatch map → `match` dispatch
```go
p.prefixParseFns[token.INT]   = p.parseIntegerLiteral   // parser/parser.go
p.infixParseFns[token.PLUS]   = p.parseInfixExpression
```
```rust
fn parse_prefix(&mut self) -> Option<Expression> {       // src/parser.rs
    match self.cur_token.token_type {
        TokenType::Int  => self.parse_integer_literal(),
        TokenType::Bang | TokenType::Minus => self.parse_prefix_expression(),
        /* ... */
    }
}
```
**Why:** a `HashMap<TokenType, fn(&mut Parser)->...>` fights the borrow checker —
you cannot borrow the map from `self` *and* call a method on `&mut self` at the
same time without cloning the map per call. `match` is the idiomatic, statically
checked dispatch.
**Discarded alternative:** the literal `HashMap` of bound method values.

### 2.4 `iota` const ladder → ordered `enum`
```go
const ( _ int = iota; LOWEST; EQUALS; LESSGREATER; SUM; PRODUCT; PREFIX; CALL; INDEX )
```
```rust
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Precedence { Lowest, Equals, LessGreater, Sum, Product, Prefix, Call, Index }
// `precedence < self.peek_precedence()` works directly on the variants.
```
**Why:** declaration order encodes the ladder; comparisons become type-safe.
**Discarded alternative:** `const LOWEST: i32 = 1; ...` — works, but loses the
"these are the only precedences" guarantee.

### 2.5 Pointer sharing/recursion → `Rc` / `Box`
```go
type Function struct { Parameters []*ast.Ident; Body *ast.BlockStatement; Env Environment }
```
```rust
pub enum Object { Function(Rc<Function>), /* ... */ }   // shared, cheap clone
pub struct PrefixExpression { pub right: Box<Expression> } // breaks the recursive type
```
**Why:** Go pointers serve two roles — *sharing* (a captured `Env`, a heap value
moved through `eval`) → `Rc`; and *breaking a recursive type* (an `Expression`
inside an `Expression`) → `Box`. Rust forces the role to be explicit.
**Discarded alternative:** clone-by-value everywhere (O(n) per pass) / a single
arena (overkill for this size).

### 2.6 String-typed const enum → `enum` + `as_str()`/`Display`
```go
type Type string
const ( IntegerType Type = "Integer"; BooleanType = "Boolean"; /* ... */ )
```
```rust
pub enum Type { Integer, Boolean, /* ... */ }
impl Type { pub fn as_str(&self) -> &'static str { match self { Type::Integer => "Integer", /* ... */ } } }
```
**Why:** the *strings* still matter (they appear verbatim in error messages like
`type mismatch: Integer + Boolean`), so the exact spellings are centralised in
`as_str`, but the *type* becomes a closed enum.
**Discarded alternative:** `struct Type(String)` newtype — keeps illegal strings
representable and loses exhaustiveness.

### 2.7 Variadic `func(x ...T)` → `fn(Vec<T>)`
```go
type BuiltinFunction func(args ...Object) Object
```
```rust
pub type BuiltinFunction = fn(Vec<Object>) -> Object;
```
**Why:** Rust has no variadics; builtins take an owned `Vec`. They are
non-capturing, so a `fn` pointer (Copy) suffices — no boxing.

### 2.8 `error` value → `Result` / `Object::Error`
```go
func runProgram(filename string) error { /* ... */ return fmt.Errorf("could not read %s: %v", ...) }
```
```rust
fn run_program(filename: &str) -> Result<(), String> { /* ... */
    .map_err(|e| format!("could not read {}: {}", filename, e))? }
```
Plus the *language-level* `object.Error` stays a normal value variant
(`Object::Error(String)`) because Monkey treats runtime errors as first-class
values printed to stdout — faithfully preserved.

### 2.9 `hash/fnv` → hand-written FNV-1a
```go
h := fnv.New64a(); h.Write([]byte(s.Value)); return HashKey{ ..., Value: h.Sum64() }
```
```rust
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes { hash ^= b as u64; hash = hash.wrapping_mul(0x100_0000_01b3); }
    hash
}
```
**Why:** a 6-line standard algorithm keeps the port **dependency-free** *and*
byte-for-byte identical to Go's hashing.
**Discarded alternative:** the `fnv` crate — an unnecessary dependency.

*(Also Adapted: struct embedding `Quote{ ast.Node }` → a named field
`Quote(Box<ast::Node>)`.)*

---

## 3. Re-architecture (1)

### 3.1 Package-level pointer-identity singletons → by-value enum variants
This is the one case where the *mechanism itself* was replaced, not merely
re-expressed.

```go
// Go: three cached singletons; equality in eval relies on POINTER IDENTITY.
var (
    NilValue   = &object.Nil{}
    TrueValue  = &object.Boolean{Value: true}
    FalseValue = &object.Boolean{Value: false}
)
func evalBangOperatorExpression(right object.Object) object.Object {
    if right == NilValue || right == FalseValue { return TrueValue }   // pointer ==
    return FalseValue
}
// the ==/!= fall-through compares interface pointers:
case operator == "==": return nativeBoolToBooleanObject(left == right)
```
```rust
// Rust: the singletons are DELETED. Booleans/Nil are plain values; equality is
// by value. The pointer-identity contract is replaced by value-identity.
fn eval_bang_operator_expression(right: Object) -> Object {
    match right {
        Object::Nil | Object::Boolean(false) => Object::Boolean(true),
        _ => Object::Boolean(false),
    }
}
// the ==/!= fall-through is reimplemented explicitly:
fn objects_equal(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Boolean(x), Object::Boolean(y)) => x == y,
        (Object::Nil, Object::Nil) => true,
        _ => false,   // compound objects: distinct, matching Go's pointer-inequality
    }
}
```
**Why re-architecture, not adaptation:** Go's design *depends on* there being
exactly one `NilValue`/`TrueValue`/`FalseValue` object so that `==` (pointer
comparison) is meaningful. Rust value enums make `Boolean(true)` canonical
everywhere, so the singletons are not just re-expressed — they are **removed**,
and the equality semantics they enabled are **reimplemented** as value equality.
The observable behaviour is identical for every reachable Monkey program (only
booleans and nil reach this path), which is exactly what the equivalence tests
and differential testing confirm.

---

## 4. Borderline cases (discussion)

- **`HashLiteral.Pairs: map[Expression]Expression` → `Vec<(Expression,Expression)>`.**
  Classified **Adapted**, but it borders on re-architecture: an Expression cannot
  be a Rust hash key (contains `f64`: not `Eq`/`Hash`), so the *data structure*
  changes from map to vector. Side effect: `String()` becomes deterministic
  (Go's map iteration order was unspecified) — a documented improvement.
- **`Environment` interface + struct + GC pointers → `Rc<RefCell<Environment>>`.**
  Counted under "pointer sharing" (Adapted), but it is the deepest adaptation:
  Go's garbage-collected shared-mutable scope becomes explicit reference counting
  + interior mutability. No behaviour changes; the *memory management strategy*
  does.

---

## 5. Why "0 unmappable" is the headline for this difficulty class

Every construct above is sequential and single-threaded with a GC→ownership
memory story that `Rc<RefCell>` covers. The categories that would *not* map
directly — goroutines/channels, `select`, `defer`, `unsafe`, `cgo`/FFI,
reflection — are **absent** from this object. CMS therefore measures *difficulty*
(how much design work the translation demanded), and for this class the answer is
"moderate idiom work, zero dead ends."

---

## 6. Reconciliation with methodology v3 (§5.1 construct table; CMS metric)

The v3 construct-mapping table uses two columns — **Strategy** ∈ {Direct,
Mechanical, Semantic, Structural, Idiomatic, Library} and **Difficulty** ∈ {Low,
Medium, High}. This document's 3-class rubric is a coarsening of that vocabulary;
the mapping is:

| This doc's class | v3 Strategy | v3 Difficulty |
|---|---|---|
| **Direct** | Direct / Mechanical / Library | Low |
| **Adapted** | Semantic / Idiomatic (and "Direct/Low" cases that still need a deliberate Rust idiom) | Low–Medium |
| **Re-architected** | Structural | Medium |

Cross-checking the specific rows v3 lists against this port (those that occur in
a C0×M0 object):

| v3 row | v3 Strategy / Difficulty | This port | Class here |
|---|---|---|---|
| Named interface → `pub trait` | Direct / Low | interface → **enum** (closed set; no `dyn`) | Adapted |
| Implicit satisfaction → explicit `impl` | Mechanical / Low | n/a (enums, not traits) | — |
| `nil` check → `Option<T>` + match/`?` | Direct / Low | `Option` (incl. Go-nil vs `NilValue`) | Adapted |
| `error` return → `Result<T,E>` + `?` | Direct / Low | `Result` / `Object::Error` | Adapted |

> **Note on one divergence.** v3 maps Go interfaces to `pub trait` (Direct/Low).
> This port instead uses **enums** for the *closed* AST/Object families (and
> `match` for dispatch), reserving traits for genuinely open extension points
> (none here). Both are valid; enums give compile-time exhaustiveness for a fixed
> variant set, which is why they were chosen (see §2.1). Under v3's vocabulary
> this is still a "Semantic/Low–Medium" adaptation, not a re-architecture.

**CMS value for this object.** The v3 CMS metric is "% of constructs mappable,
0–100 %." Every construct category here is mappable (0 unmappable), so
**CMS = 100 % mappable**, with the weighted score **0.76** reported as a
finer-grained difficulty signal. This matches v3's Class-I expectation
(`xxhash`: "CMS ≈ 100 %") and is the **first empirical confirmation of the
Class-I (C0×M0) lower bound** in the taxonomy.
