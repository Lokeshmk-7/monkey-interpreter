# Prompt log — Go → Rust transpilation of the Monkey interpreter

This is the ordered list of prompts generated and used while executing the
transpilation, grouped by the four methodology stages. Each prompt is the
self-contained instruction issued to the translation model for one unit of work.
They are reusable: re-running them in order reproduces the port.

> Convention: `{X}` denotes a slot filled from the dependency graph / prior
> outputs. "Semantic pass" = make ported tests pass; "Idiomatic pass" = refactor
> to idiomatic Rust without changing observable behaviour.

---

## Stage 0 — Intake & global setup

P0.1 — *"Enumerate every Go source file in the project, count lines, and read
each implementation and test file in full. Summarise the language features the
interpreter supports."*

P0.2 — *"Build the package dependency graph from the Go import statements.
Produce a topological ordering to use as the bottom-up translation order."*

P0.3 — *"Choose the Rust crate layout that mirrors the Go packages. Decide
single-crate-with-modules vs workspace-of-crates and justify. Emit `Cargo.toml`
with no third-party dependencies if the Go code is std-only."*

---

## Stage 1 — AST tree & report

P1.1 — *"From the Go `ast` package, extract the full AST node hierarchy:
the `Node`/`Statement`/`Expression` interfaces and every concrete node with its
fields and field types."*

P1.2 — *"Generate a self-contained, collapsible HTML report visualising (a) the
package dependency graph and translation order, (b) the AST node tree with
Go-type → Rust-type per field, (c) the object model, (d) a cross-cutting idiom
map. Output `docs/ast_report.html`."*

---

## Stage 2 — Preprocessing / normalisation

P2.1 — *"Map each Go import path to a crate-local Rust module path. Map the
multi-file `eval` package to a Rust module directory."*

P2.2 — *"Produce an identifier-normalisation table: Go CamelCase →
Rust snake_case; flag every collision with a Rust keyword or std type and choose
a rename (e.g. `Token.Type`→`token_type`, `object.String`→`Object::Str`,
`macro.go`→`macros.rs`)."*

P2.3 — *"Slice the codebase into per-module translation units along the
dependency graph so each unit fits the context window with its dependencies
treated as a fixed, already-translated interface."*

P2.4 — *"Decide the global idiom-mapping rules to apply uniformly: interface→enum,
type-switch→match, nullable-return→Option, dispatch-map→match, iota→ordered enum,
shared-mutable-scope→Rc<RefCell>, string-const→enum+as_str."*

---

## Stage 3 — Bottom-up translation (per module: semantic then idiomatic)

For each module M in [token, lexer, ast, object, parser, eval, repl, main]:

P3.M.a (Semantic) — *"Translate Go file(s) `{M files}` to `src/{M}.rs`,
preserving public behaviour and exact user-facing strings. Apply the global
idiom rules. For every type and non-trivial function, add a doc comment stating
the Go construct, the chosen Rust mapping, and the discarded alternative with the
reason it was rejected."*

P3.M.b (Tests) — *"Port the Go test file `{M}_test.go` to an inline
`#[cfg(test)] mod tests`, reproducing every table case and assertion. Replace
Go's `interface{}` expected-value tables with a small Rust enum."*

P3.M.c (Refinement loop) — *"Run `cargo test`. For each failure, feed the
compiler/test error back and fix the translation while keeping the documented
mapping. Repeat until the module's tests pass."*

Concrete module-specific prompts that were issued inside the loop:

* P3.token — *"Reproduce the exact Go token-type strings (`"IDENT"`, `"+"`,
  `"=="`, …) via `as_str`/`Display`, because the parser embeds them in error
  messages."*
* P3.lexer — *"Keep byte-level indexing (`Vec<u8>`, `u8` current char, `0` EOF
  sentinel) and a generic predicate `read`. Drop the single-impl `Lexer`
  interface in favour of a concrete struct."*
* P3.ast — *"Model `Node`/`Statement`/`Expression` as enums; box only the
  self-referential `Expression` fields; represent `HashLiteral.pairs` as a `Vec`
  of tuples and note the determinism improvement. Translate `Modify` with
  `&mut dyn FnMut(Node)->Node` and `expect_*` helpers replacing Go type
  assertions; recurse into exactly the node kinds Go's switch enumerates."*
* P3.object — *"Define `Object`/`Type` enums; wrap heavy variants in `Rc`;
  implement `Hashable` as `hash_key()->Option<HashKey>`; hand-write FNV-1a;
  model `Environment` as `Rc<RefCell<…>>` with `get`→`Option`."*
* P3.parser — *"Replace prefix/infix maps with `match` dispatch; replace `iota`
  precedences with an ordered enum; return `Option` for nil; preserve the
  two-token priming in `New`."*
* P3.eval — *"Return `Option<Object>` everywhere to model Go's nullable `Eval`
  (`None` = untyped nil ≠ `NilValue`). Split the type switch into
  `eval_program/statement/expression`. Drop the boolean/nil singletons (value
  equality). Preserve the `quote` short-circuit on the call's function literal."*
* P3.eval.builtin — *"Translate builtins as functions + a `lookup`; preserve the
  `rest` error-message copy-paste bug for I/O equivalence."*
* P3.eval.quote/macros — *"Translate the `ast.Modify` closures as `FnMut`
  capturing `env`; `convertObjectToASTNode`→`Option<Node>`; reverse-splice macro
  removal as reverse `Vec::remove`."*
* P3.repl/main — *"Map `io.Reader/Writer` to `BufRead`/`Write` generics;
  `os.Args`/`ioutil.ReadFile` to `std::env`/`std::fs`; `error` to
  `Result<(),String>`. Keep `main` free of macro processing, exactly like Go."*

---

## Stage 4 — Attention uniformity & final verification

P4.1 — *"Audit all modules for uniformity: same function order as Go, the idiom
table applied identically, a doc-comment header on every file, and tests
co-located per module. Flag and fix any drift."*

P4.2 — *"Run the full `cargo build` + `cargo test`; resolve warnings. Run a
representative Monkey program through both file mode and the REPL (with macros)
to confirm end-to-end behaviour."*

P4.3 — *"Write `docs/translation_notes.md` consolidating the methodology,
semantic mappings, preserved quirks, documented improvements, and the
file-by-file correspondence table."*

P4.4 — *"Generate this prompt log (`docs/prompts.md`)."*
