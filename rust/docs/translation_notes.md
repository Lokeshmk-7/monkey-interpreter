# Go → Rust transpilation notes — Monkey interpreter

This document records the methodology and the concrete Go→Rust mapping decisions
for porting `skatsuta/monkey-interpreter` to Rust. It is organised around the
four-step methodology supplied for the task. Per-item rationale also lives in
doc-comments at the top of each Rust source file; this file is the consolidated
view for the thesis.

The port lives in `rust/`; the original Go sources are left untouched alongside
it. Every Go test was ported and passes (`cargo test` → 48 passing).

---

## Step 1 — AST tree & visualizable report

* `docs/ast_report.html` is a self-contained, collapsible HTML report. It shows:
  1. the **package dependency graph** and the resulting bottom-up translation
     order;
  2. the **AST node hierarchy** (`Node → {Program, Statement, Expression}` and
     all concrete nodes) with each field's Go type and chosen Rust type;
  3. the **object/runtime-value model**;
  4. a **cross-cutting idiom map**.
* The tree is the artifact that drives the bottom-up approach: leaves of the
  dependency graph (`token`) are translated and verified first, then each layer
  that depends only on already-translated layers.

### Dependency graph (edges = "imports")

```
token  ← lexer
token  ← ast
ast    ← object
token, lexer, ast        ← parser
ast, object, token       ← eval
eval, lexer, object, parser ← repl
(all)                    ← main
```

Topological (translation) order: **token → lexer → ast → object → parser →
eval → repl → main**.

---

## Step 2 — Preprocessing

Go has no C-style preprocessor, so the classic preprocessing tasks (header
inclusion, macro substitution, `#ifdef` resolution) do **not** apply. The
equivalent "normalisation so the model can focus purely on translation" steps
that *were* performed:

1. **Import-graph resolution.** Mapped Go's package import paths
   (`github.com/skatsuta/monkey-interpreter/<pkg>`) to crate-local module paths
   (`crate::<module>`). One Go package ⇒ one Rust module; the multi-file `eval`
   package ⇒ a Rust module directory (`eval/{mod,builtin,quote,macros}.rs`).
2. **Identifier normalisation.** Go exported `CamelCase` ⇒ Rust `snake_case`
   functions/fields, `CamelCase` types preserved. Recorded renames where a Go
   name collides with a Rust keyword or std type:
   | Go | Rust | reason |
   |----|------|--------|
   | `Token.Type` field | `Token.token_type` | `type` is keyword-like |
   | `object.String` variant | `Object::Str` | clashes with `std::string::String` |
   | `macro.go` file | `macros.rs` | `macro` is a reserved keyword |
   | `eval.NilValue/TrueValue/FalseValue` singletons | (none) | folded into by-value enum variants |
3. **Context-window decomposition.** The codebase was sliced along the
   dependency graph into self-contained translation units (one module at a
   time), each small enough to translate and test in isolation while keeping the
   already-translated lower layers as a stable, fixed interface.
4. **Macro-system note.** Monkey's own `macro`/`quote`/`unquote` is a *runtime*
   AST-rewriting feature of the interpreted language — not a compile-time Go
   feature — so it is translated as ordinary code (`eval/macros.rs`,
   `eval/quote.rs`), not "resolved away".

---

## Step 3 — Bottom-up translation: semantic first, then idiomatic

For each module the workflow was: (a) translate for **semantic / I/O
equivalence** by porting the Go unit tests verbatim and making them pass, then
(b) apply **idiomatic** Rust constructs without changing observable behaviour.

### The load-bearing semantic mappings

| Concern | Go | Rust | Why / discarded alternative |
|---|---|---|---|
| Closed polymorphic sets (`Node`,`Statement`,`Expression`,`Object`,`Type`) | interface + type switch | `enum` + `match` | exhaustiveness checked at compile time; **discarded** `Box<dyn Trait>`+`Any` downcast (reintroduces runtime type errors) |
| Nullable `Eval` result | `object.Object` that may be untyped `nil` (≠ `NilValue`) | `Option<Object>` (`None` = Go nil) | block/program skip-logic depends on the distinction |
| Pratt dispatch tables | `map[token.Type]prefixParseFn` of bound methods | `match` in `parse_prefix`/`parse_infix` | **discarded** `HashMap<_,fn>`: can't hold `&self` (map) and `&mut self` (call) at once |
| Precedence ladder | `iota` int consts | `enum Precedence` + `derive(Ord)` | comparisons stay readable |
| Shared mutable scope + closures | `*environment` via interface, GC | `Rc<RefCell<Environment>>` | closures capture & extend their defining scope; **discarded** `Box` (no sharing) and `Arc<Mutex>` (Go type is explicitly single-threaded) |
| String-typed const enums (`token.Type`,`object.Type`) | `type T string` + consts | `enum` + `as_str()`/`Display` reproducing exact strings | error messages embed the strings verbatim → I/O equivalence; **discarded** `newtype(String)` (loses exhaustiveness) |
| `(value, ok)` returns | multiple returns | `Option<T>` | `env.Get`, `Hashable` |
| `func(...) bool` predicates passed around | first-class funcs | generic `F: Fn(u8)->bool` closures | `lexer.read` |
| FNV-1a hashing | `hash/fnv` | hand-written FNV-1a (`object::fnv1a_64`) | dependency-free **and** bit-identical |
| `map[K]V` with non-pointer iteration order needs | Go map (random order) | `HashMap` where lookup matters; `Vec` where order/keying matters (`HashLiteral.pairs`) | `Vec` fixes Go's non-deterministic `HashLiteral.String()` |
| recursive interface values (implicitly boxed) | pointers | `Box<…>` on self-referential enum fields only | avoids infinitely-sized types |

### Faithful quirks deliberately preserved

* `eval/builtin.go`'s `rest` reports `argument to ``last`` must be Array` (a
  copy-paste bug). The port keeps the identical message.
* `ast.BlockStatement` carries `expressionNode()` (not `statementNode()`) in Go;
  immaterial because it only appears as a concrete field, documented in `ast.rs`.
* `eval.go` has `<=`/`>=` arms in the integer/float infix evaluators even though
  the lexer never produces `<=`/`>=` tokens (dead but harmless). Ported as-is.
* Go's `Eval` has no `MacroLiteral` case ⇒ returns nil. Ported as
  `Expression::MacroLiteral(_) => None`.

### Documented behavioural *improvements* (no test affected)

* `HashLiteral.String()` is deterministic in Rust (`Vec` preserves source order)
  vs non-deterministic in Go (map iteration).
* A `let` whose value evaluates to Go-nil (only reachable by evaluating a bare
  `MacroLiteral` outside the REPL macro pipeline — `main` does not run macros,
  exactly like Go's `main.go`) stores `Object::Nil` so a later misuse yields a
  clean `not a function: Nil` error instead of Go's nil-pointer panic.
* Integer arithmetic uses `wrapping_*` ops so overflow wraps two's-complement
  *exactly like Go's `int64`* (Rust's plain `+`/`*`/unary-`-` panic on overflow
  in debug builds). Verified by the arithmetic-oracle property test.
* **Latent non-termination bug fixed (found by fuzzing).** Go's `skipComment` is
  `for l.ch != '\n' && l.ch != '\r' { l.readChar() }`. At EOF `l.ch == 0`, which
  is neither newline, so a `//` comment with no trailing newline (last line of a
  file, or a REPL line) loops forever. `tests/fuzz_test.rs::fuzz_lexer_always_reaches_eof`
  generated exactly this input and hung. The Rust `skip_comment` additionally
  stops at the EOF sentinel; all newline-terminated comments (every Go test)
  behave identically. This is the headline demonstration that the property/fuzz
  layer adds value beyond example-based tests.

---

## Step 4 — Attention uniformity across the project

Measures taken so quality does not degrade across modules / large context:

* **One Go package ⇒ one Rust module**, and within a module functions appear in
  the **same order** as the Go source, so a reviewer can diff side-by-side.
* A **fixed naming/idiom table** (above) applied identically everywhere — e.g.
  every Go interface→enum, every `(v, ok)`→`Option`, every type-switch→`match`.
* **Tests co-located and ported 1:1.** Go's in-package (white-box) tests become
  `#[cfg(test)] mod tests` in the same file, exercising the same entry points
  and the same table cases. This pins behaviour per-module independently.
* **Per-item doc comments** stating the Go construct, the Rust mapping, and the
  discarded alternative — uniform structure at the top of every file and on
  non-obvious items.
* **Verification gate:** `cargo build` + `cargo test` after each layer; the final
  suite (48 tests) covers token, lexer, ast (+modify), object, parser, eval,
  quote, and macros.

---

## File-by-file correspondence

| Go file | Rust file | Notable transforms |
|---|---|---|
| `token/token.go` | `src/token.rs` | const strings → `enum`+`as_str`; keyword map → `match` |
| `lexer/lexer.go` | `src/lexer.rs` | interface dropped; byte input as `Vec<u8>`; predicate `read` generic |
| `ast/ast.go` | `src/ast.rs` | interfaces → enums; `String()`→`Display`; `Box` for recursion |
| `ast/modify.go` | `src/ast.rs` (`modify`) | `ModifierFunc` → `&mut dyn FnMut(Node)->Node`; type assertions → `expect_*` |
| `object/object.go` | `src/object.rs` | `Object`/`Type` enums; `Hashable`→`hash_key()`; FNV by hand |
| `object/environment.go` | `src/object.rs` (`Environment`) | interface → `Rc<RefCell<…>>` |
| `parser/parser.go` | `src/parser.rs` | dispatch maps → `match`; `iota` → `enum Precedence`; nil → `Option` |
| `eval/eval.go` | `src/eval/mod.rs` | `Eval` → `Option<Object>`; type switch → nested `match` |
| `eval/builtin.go` | `src/eval/builtin.rs` | builtins map → `lookup` + `fn`s |
| `eval/quote.go` | `src/eval/quote.rs` | modifier closure; `convert…`→`Option<Node>` |
| `eval/macro.go` | `src/eval/macros.rs` | reverse-splice → reverse `Vec::remove`; `(macro, ok)` → `Option<Rc<Macro>>` |
| `repl/repl.go` | `src/repl.rs` | `io.Reader/Writer` → `BufRead`/`Write` generics |
| `main.go` | `src/main.rs` | `os.Args`/`ReadFile` → `std::env`/`std::fs`; `error` → `Result<_,String>` |
