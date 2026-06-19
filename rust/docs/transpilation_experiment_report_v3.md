# Go → Rust Transpilation: A Reproducible Experiment Report

### LLM-assisted, verification-gated migration of a 4.8 KLOC interpreter

*Structured per Jedlitschka, A. & Pfahl, D., "Reporting Guidelines for Controlled
Experiments in Software Engineering," ISESE 2005 — written to serve two readers:
a **thesis examiner** (rigour, threats to validity, reproducibility) and a
**project manager / engineer** scoping a Go→Rust migration for this class of
system (cost, risk, what to budget for). Manager- and engineer-facing call-outs
are boxed throughout.*

> **This file supersedes the earlier draft `experiment_report.md`.** It adds the
> dual-audience framing, the full metric framework mapped to the methodology
> document (§5.2 Approach 1 quality metrics, §7 Approach 2 process metrics), the
> elaborated central implication, the oracle explanation, and the Windows
> reproduction package.

> **Study-type caveat.** Jedlitschka & Pfahl describe *controlled experiments*
> with human subjects and multiple objects. This is a single-object
> feasibility/experience study (n = 1 system, producer = one LLM agent). The
> reporting *structure* is followed in full; where an element presupposes human
> subjects or replication it is kept as a heading and its inapplicability stated.

---

## How to read this report

| You are a… | Read |
|---|---|
| **Project manager** scoping a migration | Abstract, §1.3, the boxed *Manager take-aways*, §5 (metrics), §6.1, §7 |
| **Engineer** doing the work | §3.5 (procedure), `construct_mappability.md`, the *Engineer take-aways*, §6.4 (lessons) |
| **Thesis examiner / researcher** | All sections; §3 (design), §5 (analysis), §6.2 (threats), Appendices |

---

## Metrics scorecard (one glance)

*Object: Monkey interpreter · **Class I (C0×M0), TCS ≈ 3** — the easiest taxonomy
cell. Read every number as a **Class-I** result (§3.3.1).*

| **A — Transpiled-code quality** *(the product)* | Value | | **B — Transpilation-process quality** *(the process)* | Value |
|---|---|---|---|---|
| Functional Correctness (FC) | 94/94 tests, **0 diffs** | | Construct Mappability (CMS) | **100 % mappable** (score 0.76) |
| Memory Safety (MS — STR) | **100 % safe** (0 `unsafe`) | | Automation Failure Rate (AFR) | **0.44 %** lines repaired |
| Idiomacy (ID) | **0** Clippy warn/KLOC | | Transpilation Effort (TE) | ~1× code; parser/eval-heavy |
| Maintainability (MT — AO) | 27 % annotation | | Toolchain Migration (TME) | near-zero (0 deps) |
| 3rd-party deps (TD) | **0 crates**, EC 100 % | | | |
| LOC Delta (LOCD) | **1.02×** code | | | |

> **Manager one-liner.** Easiest class; the product is fully safe, idiomatic, and
> behaviourally identical; the process needed <0.5 % rework and zero dependency
> work. Budget the **test harness**, not human code review (§6.2).

---

## Structured Abstract

- **Background.** Migrating garbage-collected, interface-based Go to
  ownership-based Rust is a recurring modernization task. LLMs automate much of
  it, but the *boundary of safe automation* and the *cost/quality* profile are
  unquantified for the common "sequential, no-`unsafe`, no-FFI" class.
- **Objective.** Quantify, for one representative interpreter (4.8 KLOC Go incl.
  tests), (i) behavioural equivalence, (ii) construct mappability, (iii) the
  automated effort and repair rate, and (iv) toolchain-migration cost — using a
  metric framework that scores both the **product** and the **process**.
- **Method.** Bottom-up, dependency-ordered translation; every Go test ported as
  an oracle; **differential testing** of the two compiled interpreters on
  identical programs; **property/fuzz** testing with an independent arithmetic
  oracle.
- **Results.** 94 Rust tests pass; Go and Rust binaries are **byte-identical** on
  the differential program and output is deterministic. **0 / 22** construct
  categories were unmappable. Product metrics: **0 `unsafe`** (Safe Transpilation
  Ratio = 100 %), **0 Clippy warnings/KLOC** (vs a 21/KLOC baseline), code-LOC
  delta **1.02×**, **0** third-party dependencies. Process metrics:
  lines-corrected ratio **0.44 %**, Construct-Mappability Score **0.76**, near-zero
  toolchain cost. The only genuine semantic gaps (integer-overflow wrapping; a
  comment-at-EOF non-termination bug inherited from Go) were caught **only** by
  the property/fuzz layer.
- **Conclusion.** For this class, Go→Rust transpilation is almost fully
  automatable; the residual human boundary is *semantic adjudication of
  unspecified edges*, best enforced by a differential + property harness.

> **Manager take-away (TL;DR).** For a sequential, std-library Go service of this
> size, budget the migration as *"~1× the code, mostly mechanical, plus a test
> harness."* The harness — not human code review — is the deliverable that
> de-risks it. Expect ~0.5 % of lines to need correction and **no** dead-end
> constructs.

---

## 1. Introduction

### 1.1 Problem statement
Hand-porting Go to Rust is costly; the open questions are *how much* an LLM can
automate and *where* humans must intervene. Reports rarely separate (a)
mechanical translation, (b) idiom mapping, and (c) semantic adjudication, nor
quantify the residual repair effort or the product quality that results.

### 1.2 Research objectives (GQM)
> **Analyze** LLM-assisted Go→Rust transpilation **for the purpose of**
> characterising automatability and resulting quality **with respect to**
> behavioural equivalence, construct mappability, repair rate, and
> toolchain-migration cost **from the viewpoint of** an engineer/researcher **in
> the context of** a 4.8 KLOC interpreter with no concurrency, `unsafe`, FFI, or
> third-party dependencies.

### 1.3 Context
- **Object:** `skatsuta/monkey-interpreter` (after Thorsten Ball, *Writing an
  Interpreter in Go*), extended with float literals, line comments, and a
  `quote`/`unquote`/`macro` system. 8 packages, 13 source files.
- **Difficulty class:** purely sequential; value/pointer + GC memory model;
  standard library only; **no** goroutines/channels/`defer`/`unsafe`/cgo.
- **Producer:** an LLM coding agent driving an edit→build→test loop; the
  verification gate is `go test` + `cargo test` + differential runs.

> **Manager take-away.** The findings below are scoped to *this class*. They do
> **not** transfer to services using goroutines/channels, `unsafe`, cgo/FFI,
> reflection, or heavy third-party trees — those move the automation boundary and
> must be scoped separately (see §7.4).

---

## 2. Background

### 2.1 Terminology
- *Transpilation*: source-to-source translation preserving observable behaviour.
- *Behavioural / I-O equivalence*: identical stdout, exit code, and error text
  for identical input.
- *Oracle*: a mechanism that decides whether an output is correct (see §6.5).
- *Construct mappability*: whether a source construct has an idiomatic target
  counterpart (Direct / Adapted / Re-architected / Unmappable).

### 2.2 Related work
Rule-based transpilers (e.g. C2Rust) guarantee a syntactic translation but emit
non-idiomatic, frequently `unsafe` Rust; LLM-based translation emits idiomatic
Rust without soundness guarantees. This study is in the second camp and *recovers*
assurance with a verification harness (ported oracle + differential + property
fuzzing).

---

## 3. Experiment Planning

### 3.1 Goals & hypotheses
| ID | Hypothesis |
|----|------------|
| H1 | The Rust port is I-O equivalent to Go on the oracle and a differential battery. |
| H2 | The Rust interpreter is deterministic (no map-order leakage into output). |
| H3 | No Go construct in this class is unmappable. |
| H4 | Source-side repair after automated generation is small; residual gaps are *semantic*, not syntactic. |
| H5 | Build/CI migration cost is small and mechanical. |
| H6 | The product is memory-safe and idiomatic (0 `unsafe`, low lint density). |

### 3.2 Variables
- **Independent / fixed:** source program; target = Rust 2021 (stable 1.82);
  strategy = bottom-up, enum-over-trait, `Rc<RefCell>` shared scope, 0 crates.
- **Dependent (measured):** the product- and process-metric frameworks in §3.7.

### 3.3 Experimental units / objects
Single object, decomposed into Go packages → 8 translation *phases*
(token, lexer, ast, object, parser, eval, repl, main).

### 3.3.1 Taxonomy position (methodology v3): **Class I — (C0 × M0), TCS ≈ 3**
Locating the object in the v3 C×M taxonomy is essential to scope the findings.
Static signals computed from the Go source:

| Axis | Signals (per KLOC) | Value | Level |
|---|---|---|---|
| **C (concurrency)** | goroutines, channels, `select`, mutex fields | **all 0** | **C0** (C-Score ≈ 0) |
| **M (memory ownership)** | `unsafe` ops 0; goroutine-ptr args 0; cross-thread shared-mutable (sync.Map/atomic/mutex) 0; pointer receivers idiomatic; ~6 effectively-immutable package vars | **low** | **M0** (M-Score ≈ low single digits) |

⇒ **TCS = 0.6·C + 0.4·M + (C·M)/200 ≈ 3** → **Class I**, the *same cell as the
methodology's `xxhash` lower-bound project* ("trivial without Go idioms;
CMS ≈ 100 %, AFR ≈ 0 %, EC = 100 %, DGS = 0"). §5 reports whether the measured
outcomes match that Class-I prediction (they do). **All conclusions in this
report are Class-I conclusions** and must not be read across to Classes II–IV
(ownership-heavy, concurrent, or `unsafe` projects), where the automation
boundary moves (§6.3, §7.4).

> **Manager take-away.** Before quoting a Go→Rust migration, *classify the
> codebase first* (C×M / TCS from Go static analysis). This case study is the
> **easiest** class (C0×M0). A service with goroutines+channels (C2) or
> `unsafe`/shared-mutable state (M2–M3) is a different cost régime — budget it
> from its own class, not from these numbers.

### 3.4 Subjects/participants
Not applicable (no human subjects; producer is one LLM agent). Retained per
guideline.

### 3.5 Materials, instrumentation & procedure (the *treatment*)
Toolchains: Go 1.22.5, Rust/Cargo 1.82.0. Oracle: all 11 Go `_test.go` files,
ported. Harness: differential (`fc`/`diff` of two compiled interpreters) +
property/fuzz (seeded PRNG, no deps). Prompt protocol: `prompts.md`.

Procedure:
1. **Intake & DAG** — read sources; build the import DAG; topologically order it.
2. **AST/report** — emit the node/dependency report (`ast_report.html`).
3. **Preprocess** — import→module map, identifier normalisation, fix the global
   idiom rules (interface→enum, type-switch→match, shared-scope→`Rc<RefCell>`,
   dispatch-map→match, nil→`Option`).
4. **Per phase, bottom-up** — translate (semantic first), port the phase's tests,
   iterate on `cargo build`/`cargo test` to green, then idiomatic polish.
5. **Verify** — full suite + differential + property/fuzz.

> **Engineer take-away.** The single highest-leverage practice is **port the
> source's tests first** and run them as you translate each module bottom-up.
> They pin equivalence per module and force the exact-error-string decisions.

### 3.6 Design
Descriptive, single-object. Each hypothesis has a pre-stated acceptance criterion
(§5.3). Metric formulae are fixed in Appendix A before values are reported.

### 3.7 Metric framework — two dual-audience buckets

The methodology v3 lists nine "Approach-1" metrics (§5.2) plus an Approach-2
system dimension set (§7). For this report they are **re-organised into the two
buckets that match the dual-audience use case** — *what you migrated* vs *how the
migration went* — because that split is what a manager and an engineer actually
reason about. The mapping back to v3's own labels is given in the right-hand
column, so nothing is lost for the thesis. **Runtime Performance (RP) and
Concurrency-Idiom Density (CID) are out of scope** by request (CID is also moot
here: this is a C0 project, so CID ≡ 0).

> **The two questions the buckets answer.**
> **A. Transpiled-Code Quality** = "Is the *product* good?" (correctness, safety,
> maintainability, dependencies, readability, idiomaticity, size). Read by anyone
> who must *own and run* the Rust afterwards.
> **B. Transpilation-Process Quality** = "Was the *migration* cheap, predictable,
> and automatable?" (effort, repair rate, construct difficulty, toolchain cost).
> Read by whoever must *plan and budget* the migration.

**Bucket A — Transpiled-Code Quality (the product).** *(v3 Approach-1 product metrics)*

| Metric (symbol) | Operational definition in this study | v3 origin |
|---|---|---|
| **Functional Correctness (FC)** | (# oracle tests passed, # differential mismatches, property-fuzz pass); pass-rate 0–100 % | §5.2 #1 |
| **Memory Safety (MS)** | **Primary — Safe-Transpilation Ratio (STR)** = % of code LOC *not* inside `unsafe`. **Secondary — Unsafe-Block Ratio (UBR)** = `unsafe` blocks ÷ functions (the v3-native MS, scale 0–1). *STR is the recommended headline: a coverage-style %, manager-intuitive, robust when functions are few; UBR is kept for v3 continuity.* | §5.2 #2 (MS/UBR) |
| **Maintainability (MT)** | **Annotation Overhead (AO)** = doc/comment LOC ÷ code LOC; **Avg Cyclomatic Complexity (proxy)** = decision-points ÷ functions + 1 | §5.2 #3 (CC + AO%) |
| **3rd-Party Dependencies (TD)** | **DCD** direct crates; **DGS/TDC** transitive graph size; **EC%** ecosystem coverage (% of needed deps available); license compliance | §5.2 #5 / §7 D1 (EC, DGS, DCD) |
| **Readability (RS)** | 2 reviewers, 1–5 Likert, Cohen's κ agreement *(manual; Appendix C)* | §5.2 #3 (RS 1–5) |
| **Idiomacy (ID)** | **Idiomatic-Lint Density** = Clippy warnings ÷ KLOC, vs a 21/KLOC baseline | proposal/v3.1 |
| **LOC Delta (LOCD)** | Rust LOC ÷ Go LOC (verbosity signal), code-LOC and physical | proposal/v3.1 |
| *(proposed 9th)* **Behavioural-Equivalence Coverage (BEC)** | identical-behaviour count ÷ behaviours in the differential+oracle suite | new |

**Bucket B — Transpilation-Process Quality (the process).** *(v3 Approach-1 process metrics + Approach-2 §7)*

| Metric (symbol) | Operational definition in this study | v3 origin |
|---|---|---|
| **Transpilation Effort (TE)** | effort by phase; LLM producer ⇒ logged as generated-LOC + build/test iterations (objective), with a human-hours column for a manual daily log (subjective) | §5.2 #8 |
| **Automation Failure Rate (AFR)** | transpiler-side lines corrected ÷ generated code-LOC; *median of 3 generation runs* (protocol; one run observed) | §5.2 #7 |
| **Construct Mappability (CMS)** | construct classification → Direct/Adapted/Re-arch/Unmappable; reported as *mappable %* and a weighted score. **A difficulty metric, not a product metric** (placed in Process deliberately, per the v3.1 note). | §5.2 #6 |
| **Toolchain Migration Effort (TME)** | build-manifest + CI/CD conversion cost (lines changed; dependency-porting lines) | §7 D2/D5 |

> *Deferred (out of scope here): Runtime Performance (RP, §5.2 #4 — Criterion.rs
> vs tikv/raft-rs) and Concurrency-Idiom Density (CID, §5.2 #9 — ≡ 0 for a C0
> project). Both are listed in §7.4 future work.*

---

## 4. Execution

### 4.1 Preparation
The Go module was made locally buildable (`go.mod` module path set to the import
prefix `github.com/skatsuta/monkey-interpreter`; an auto-generated manifest had
resolved imports to the *published* package). A Rust crate (`rust/`) mirrored the
package layout.

### 4.2 Data collection performed
LOC via `wc`/PowerShell (code vs comment vs blank); iteration/repair events from
the edit/build/test trace; equivalence via `cargo test` counts + `fc`/`diff`;
determinism via 3× repetition; idiomacy via `cargo clippy`; safety via `unsafe`
grep.

### 4.3 Deviations
- Oracle tests were **moved** from inline modules to a `tests/` integration
  directory (on request), which *strengthened* equivalence (public-API only).
- Two overflow-wrapping edits were made proactively while building the arithmetic
  oracle, marginally before the property test would have forced them — recorded
  as repairs for honesty.

---

## 5. Analysis

### 5.0 Measurement procedure & data provenance
Section-5 values come from **two provenances**, and the distinction is a
reportable property of the study:

- **Bucket A (product) — measured from the final artifact**, hence fully
  re-computable by anyone with the repo + toolchain. The script
  `metrics.ps1` (Appendix B) regenerates every Bucket-A value.
- **Bucket B (process) — observational, captured from the development trace**
  (edit/build/test log + git history). It is *not* derivable from the final code
  alone; `metrics.ps1` recomputes the artifact-backed parts (TE per-phase LOC,
  TME) and prints the log-derived parts (AFR, CMS, TE-hours) with their captured
  values and the protocol to reproduce them.

| Metric | How captured | Re-computable from code? |
|---|---|---|
| FC — tests | `cargo test`, sum `test result: ok. N passed` | ✅ |
| FC — differential | build Go + Rust, run `prog.monkey` through both, compare stdout | ✅ (needs `go`) |
| MS — STR / UBR | regex `\bunsafe\b` (→0) and `\bfn \w+` (→132) over `rust/src`; STR = 1−unsafe/code, UBR = unsafe/fns | ✅ |
| MT — AO | comment-LOC ÷ code-LOC; code = non-blank − comment | ✅ |
| MT — CC (proxy) | decision-point regex (`if`/`while`/`for`/`match`/`=>`/`&&`/`\|\|`) ÷ fns + 1. *Tool-sensitive proxy: ripgrep→641 (5.9), PowerShell→660 (6.0); same order of magnitude.* | ✅ (±tool) |
| TD — DCD/DGS/EC | `Cargo.toml [dependencies]` count; `cargo tree`; EC=100 % when DCD=0 | ✅ |
| ID — lint density | `cargo clippy --all-targets`, count `^warning:` minus summary, ÷ KLOC | ✅ |
| LOCD | same LOC method on Go impl (`*.go` ∖ `*_test.go` ∖ `rust/`); ratio | ✅ |
| TE — phase LOC | per-file non-blank/comment LOC | ✅ |
| TE — iterations/hours | manual daily log (Appendix A) during the run | ❌ (log) |
| AFR | edit-trace count of transpiler-side lines changed after first green compile ÷ code-LOC; optional git estimate vs a `green-baseline` tag | ❌ (log/git) |
| CMS | manual classification of each construct category (`construct_mappability.md`) → weighted formula | ❌ (analyst) |
| TME | build/CI manifest line deltas | ✅ |

> **Manager take-away.** Anyone can re-verify the *product* claims in one command
> (`metrics.ps1`); the *process* claims require trusting (or re-deriving from git)
> the development log. For an external audit, the differential test (FC) and the
> `unsafe`/clippy counts (MS/ID) are the cheapest high-trust checks.

### 5.1 Descriptive statistics — values

**Bucket A — Transpiled-Code Quality (the product):**

| Metric | Value | Reading |
|---|---|---|
| FC — Functional Correctness | 94/94 oracle tests pass; **0** differential mismatches; property-fuzz pass | functionally equivalent |
| MS — STR (primary) | **100 %** (0 `unsafe` lines) | fully safe Rust |
| MS — UBR (v3-native) | **0.000** (0 `unsafe` blocks / 132 fns) | — |
| MT — AO | **26.9 %** (554 doc-LOC / 2056 code-LOC) | deliberately documentation-heavy |
| MT — CC (proxy) | **≈ 5.9–6.0** avg (decision-points ÷ 132 fns + 1; ripgrep 641→5.9, `metrics.ps1` 660→6.0) | low–moderate complexity |
| TD — DCD / DGS / EC% | **0 / 0 / 100 %** (0 crates ⇒ ecosystem coverage trivially complete) | std-library only |
| TD — License | MIT + Rust std; **no copyleft introduced** | compliant |
| RS — Readability | *manual* (Appendix C); self-rating 4.5/5 pending review | — |
| ID — Lint density | **0 warnings/KLOC** (vs 21/KLOC baseline) | highly idiomatic |
| LOCD | **1.02×** code (2056/2017); 1.09× physical (2938/2693) | ~1:1, no bloat |
| BEC (proposed) | 18/18 differential behaviours + 94/94 oracle | full on tested behaviours |

**Bucket B — Transpilation-Process Quality (the process):**

| Metric | Value | Reading |
|---|---|---|
| TE — effort by phase | see table below (generated-LOC + iterations) | front-loaded on parser/eval |
| AFR | **0.44 %** (≈9 transpiler-side lines / 2056) ; ≈ **2 defect-sites/KLOC** | low |
| CMS | **mappable 100 %**; weighted **0.76** (Direct 12 / Adapted 9 / Re-arch 1 / Unmappable 0) | moderate idiom work, 0 dead ends |
| TME | `go.mod`(3)→`Cargo.toml`(~25); `go test`→`cargo test`; **0** dep work; ~15-line CI | near-zero |

> **Class-I prediction check (methodology §3.5 / §8).** v3 predicts for a Class-I
> (C0×M0) project: *CMS ≈ 100 %, AFR ≈ 0 %, EC = 100 %, DGS = 0.* Measured here:
> **CMS mappable = 100 %, AFR = 0.44 %, EC = 100 %, DGS = 0.** The case study
> **confirms the Class-I lower-bound prediction** — the first empirical data point
> for that cell (alongside the methodology's `xxhash`).

**TE — effort by phase** (objective proxy; human-hours column for a manual log):

| Phase | Rust code LOC | Doc LOC | Build/test iterations | Human hrs* |
|---|---|---|---|---|
| token | ~120 | high | 1 | _log_ |
| lexer | ~150 | med | 1 (+1 bug-fix later) | _log_ |
| ast (+modify) | ~330 | high | 1 | _log_ |
| object (+env) | ~210 | high | 1 | _log_ |
| parser | ~430 | med | 1 | _log_ |
| eval (×4 files) | ~560 | high | 1 (+2 fidelity edits) | _log_ |
| repl + main | ~100 | med | 1 | _log_ |
| tests + harness | (2440 test LOC) | — | 3 | _log_ |
| *Totals* | **2056 code** | **554** | — | — |

\* The producer is an LLM; *Human hrs* is the column a human operator fills with a
daily time log (TE is "subjective" per the methodology).

> **Manager take-away.** Effort concentrates on **parser** and **eval** (the
> semantically rich phases). Everything else is near-mechanical. Test/harness
> construction is a real, separately-budgeted line item — and it is where the
> assurance comes from.

### 5.2 Data-set preparation
No outlier removal; all events from one development trace are reported. AFR's
3-run-median protocol is documented though only one generation run was executed.

### 5.3 Hypothesis evaluation
| H | Criterion | Result | Verdict |
|---|---|---|---|
| H1 | 0 differential mismatches & all oracle tests pass | 94/94 + 0 | **supported** |
| H2 | identical output across repeats; no map-order leakage | 3/3; `HashLiteral` made deterministic | **supported** |
| H3 | 0 unmappable constructs | 0/22 | **supported** |
| H4 | lines-corrected < 5 %; residual gaps semantic | 0.44 %; both gaps semantic | **supported** |
| H5 | build/CI mostly mechanical | yes; 0 dep work | **supported** |
| H6 | 0 `unsafe`; low lint density | STR 100 %; 0 warnings/KLOC | **supported** |

---

## 6. Interpretation

### 6.1 Evaluation of results and implications
Code-LOC expansion is ≈ 1.02× (≈ 1:1); the visible size growth is documentation
and *added* tests, not translated-code bloat. The example-based oracle passed at
first green compile — the LLM reproduced **specified** behaviour reliably. The
defects that escaped were **unspecified-edge** semantics.

### 6.2 The central implication — elaborated
> **The automation boundary is not syntax or idiom; it is semantic adjudication
> of *unspecified edges*, and the cheapest way to police that boundary is a
> differential + property-based harness, not human diff review.**

Unpacking this, with the evidence:

1. **What the LLM got right, first time.** All 48 ported example tests passed at
   the first successful compile. Syntax, the idiom mappings (interface→enum,
   dispatch→match, `Rc<RefCell>`), and every *named* behaviour were correct
   without iteration. So the bottleneck is **not** translation accuracy on
   specified behaviour.
2. **What escaped, and why it is a *class*.** The genuine defects were
   *latent contracts* — behaviours emergent from the **source runtime**, never
   written in code or tests:
   - **Integer overflow:** Go's `int64` wraps silently; Rust's `+`/`*`/unary `-`
     *panic* in debug. The line `l + r` translates to `l + r` — it *reads*
     correct. Only an input that overflows reveals the divergence.
   - **Comment-at-EOF non-termination:** Go's `for l.ch != '\n' && l.ch != '\r'`
     loops forever at EOF; the faithful translation inherited it. Only a `//`
     comment without a trailing newline triggers it.
   These are invisible to *both* a human diff-reviewer and an example test,
   because nothing in the source or its tests *names* the edge.
3. **Why human diff-review is the wrong control.** A reviewer comparing
   `l + r` → `l + r` sees a faithful line and approves it. Diff-review scales
   with *lines*, but the risk lives in *behaviours per input*, which diffs do not
   surface. Reading more diff buys little assurance against latent contracts.
4. **Why differential + property testing is the right control.** A **differential
   oracle** (the Go binary) and a **property oracle** (an independent evaluator,
   §6.5) check *behaviour over inputs*, so they catch exactly the latent-contract
   class. In this study the overflow gap was forced by the arithmetic property
   oracle and the comment hang by the lexer-termination property — *neither* by
   the ported example suite.
5. **Consequences.**
   - *For managers:* fund the **harness** (oracle port + differential runner +
     fuzz), not line-by-line human review. The harness is the risk-reduction
     deliverable; translation is cheap.
   - *For engineers:* treat every place the Rust default differs from the Go
     runtime (overflow, division, float formatting, NaN, map order, EOF) as a
     **latent-contract checklist** and write a property/differential test for
     each.
   - *For researchers:* the meaningful dependent variable is **latent-contract
     coverage of the harness**, not LLM translation accuracy.

> **Engineer take-away — latent-contract checklist for Go→Rust:** integer
> overflow (`wrapping_*`), integer division/`%` by zero, `i64::MIN / -1`, float
> formatting & NaN/Inf, map/iteration order in output, string byte-vs-char
> length, EOF/sentinel loop conditions, and `nil` vs a null-object singleton.

### 6.3 Threats to validity
- **Construct.** "Effort" is proxied by LOC/iterations, not human hours (producer
  is an LLM). The CMS weights are chosen; raw counts are also reported. "Idiomacy"
  uses Clippy's default lints as a proxy for human-judged idiom.
- **Internal.** The same agent wrote code *and* its oracle tests; a shared
  misconception could pass both. **Mitigated** by (a) **differential testing vs an
  independent implementation** (the Go binary) and (b) the **independent property
  oracle** — and indeed both real defects came from these, not the ported suite.
- **External.** Single object, one difficulty class. **No** concurrency, `unsafe`,
  cgo/FFI, reflection, or third-party trees; findings do not extend there.
- **Conclusion.** n = 1; descriptive only, no inferential statistics.

### 6.4 Lessons learned
1. Bottom-up + per-phase tests localise defects to one module.
2. Port the source's tests verbatim first — cheapest equivalence oracle.
3. Differential + property testing is non-negotiable for unspecified edges.
4. Preserve quirks deliberately, fix bugs deliberately — and **record both**
   (kept: the `rest` error-message quirk; fixed: the comment-EOF hang).
5. Normalise reserved-word / std-name clashes (`type`, `String`, `macro`) up front.

### 6.5 What an "oracle" is, and the "property oracle" used here
A **test oracle** is any mechanism that decides whether the output for a given
input is *correct*. The hard part of testing is the **oracle problem**: knowing
the expected answer. This study uses three oracle kinds, in increasing power:

1. **Example-based oracle** — fixed *(input → expected output)* pairs (the ported
   Go tests). Cheap, but only covers the cases someone wrote down.
2. **Differential oracle** *(a.k.a. back-to-back testing)* — the **Go binary is
   the oracle**: for *any* input, the Rust output must equal the Go output. This
   needs no hand-written expected value, so it covers arbitrary inputs — including
   the unspecified edges — provided the inputs reach them.
3. **Property / metamorphic oracle** — an **invariant that must hold for all
   inputs**, expressed without naming expected outputs. Used here:
   `lex(x)` always terminates at EOF; `parse∘to_string` is idempotent;
   and the **independent arithmetic evaluator** (below).

**The independent arithmetic evaluator (the property oracle in `fuzz_test.rs`).**
The interpreter under test is *one* implementation of "evaluate a Monkey
arithmetic expression." The property oracle is a *second, deliberately trivial*
implementation that shares **no code** with it:

```rust
enum IntE { Num(i64), Add(Box<IntE>,Box<IntE>), Sub(..), Mul(..), Neg(Box<IntE>) }

impl IntE {
    fn render(&self) -> String { /* "(a + b)", "(-a)", ... -> Monkey source */ }
    fn eval(&self)   -> i64    { /* directly, with wrapping_* i64 ops */ }
}
```
For thousands of randomly generated expressions the test:
(a) `render`s the expression to Monkey source, (b) runs it through the **full**
lexer→parser→interpreter, and (c) asserts the interpreter's result equals
`IntE::eval()`. Because the two evaluators are independent code paths, agreement
across a large random sample is strong evidence the interpreter's arithmetic is
correct — and any disagreement is an automatically-minimisable counterexample
(the PRNG seed is fixed, so it reproduces). This is precisely the oracle that
forced the overflow-wrapping fidelity; an example-based oracle never would.

> **Manager take-away.** Three oracle types, increasing assurance and cost:
> *examples* (cheap, shallow) → *differential vs the old system* (medium, broad)
> → *properties/invariants* (higher skill, catches the edges). For a migration,
> the differential oracle against the **existing** system is the highest
> value-for-effort; add a few properties for the known latent-contract risks.

---

## 7. Conclusions and Future Work

### 7.1 Summary
An LLM-assisted, dependency-ordered transpilation achieved byte-identical
behaviour (94 tests + differential battery), ~1.02× code expansion, 0 `unsafe`,
0 Clippy warnings/KLOC, 0 unmappable constructs, and < 0.5 % source-line repair —
with the only semantic gaps caught by a property/fuzz layer.

### 7.2 Relation to existing evidence
Rule-based transpilers maximise *guarantees*; LLMs maximise *idiomaticity*. Adding
a differential + property harness lets the LLM route approach rule-based assurance
on a small, sequential object.

### 7.3 Limitations
Single object; no concurrency/`unsafe`/FFI/third-party deps; effort not in human
hours; CMS weights and the 21/KLOC idiomacy baseline are chosen, not derived;
Runtime Performance not measured (out of scope).

### 7.4 Future work
Repeat on objects with (a) goroutines/channels (→ `std::thread`/`mpsc`/async),
(b) third-party dependencies (crate-equivalence search), (c) `unsafe`/cgo, to map
how the automation boundary moves; add **Runtime Performance** (Criterion.rs vs a
Go baseline) and **Concurrency-Idiom Density**; automate metric collection into CI
(`reproduce.bat` → a workflow); add a coverage-guided `cargo-fuzz` harness.

---

## Acknowledgements
Toolchains: Go 1.22.5, Rust 1.82.0. Object: `skatsuta/monkey-interpreter`.

## References
- Jedlitschka, A., Pfahl, D. *Reporting Guidelines for Controlled Experiments in
  Software Engineering.* ISESE 2005, IEEE.
- Ball, T. *Writing an Interpreter in Go*; *The Lost Chapter: A Macro System.*
- Basili, V., Caldiera, G., Rombach, H.D. *The Goal Question Metric Approach.*

---

## Appendix A — Metric formulae & the daily effort log

```
code_LOC(x)          = nonblank_lines(x) − comment_lines(x)
FC                   = (oracle_pass, differential_mismatches, property_pass)
STR (MS, primary)    = 1 − (LOC_in_unsafe_blocks / code_LOC)            -> 100%
UBR (MS, secondary)  = unsafe_blocks / function_count                   -> 0.000
AO  (MT)             = doc_comment_LOC / code_LOC                       -> 26.9%
CC_proxy (MT)        = decision_points / functions + 1                  -> ~5.9-6.0 (tool-sensitive)
EC  (TD, v3 = ecosystem coverage) = mappable_deps / required_deps     -> 100% (0/0 ⇒ trivially complete)
DCD (TD)             = direct_crate_dependencies                       -> 0
DGS (TD)             = transitive_dependency_graph_size                -> 0
ID  (Idiomacy)       = clippy_warnings / (code_LOC/1000)                -> 0 / KLOC
LOCD                 = rust_LOC / go_LOC          (code: 1.02, phys: 1.09)
AFR                  = transpiler_side_lines_corrected / generated_code_LOC  (median of 3 runs)
CMS_mappable         = mappable_categories / categories                -> 100% (22/22)
CMS_weighted         = (1.0·Direct + 0.5·Adapted + 0.25·ReArch) / categories -> 0.76
TME                  = manifest_lines_changed + ci_lines_changed + dep_porting_lines
```

**Daily effort log template (TE, subjective):**
| Date | Phase | Activity (translate / port-tests / repair / review) | Hours | Notes |
|------|-------|------------------------------------------------------|-------|-------|

## Appendix B — Reproduction package (Windows)

**B.1 `metrics.ps1` — regenerate the Section-5 metrics.** Run from the repo root:
```powershell
powershell -ExecutionPolicy Bypass -File .\metrics.ps1
```
It prints both buckets and writes `metrics_report.csv`. It recomputes every
Bucket-A (product) metric and the artifact-backed Bucket-B parts (TE-LOC, TME)
from the code + toolchain, and echoes the log-derived parts (AFR, CMS, TE-hours)
with their captured values. Switches: `-SkipTests`, `-SkipDifferential`,
`-SkipClippy`, `-Csv <path>`. To auto-estimate AFR from git, tag the first green
build (`git tag green-baseline`) and re-run.

**B.2 `reproduce.bat` — end-to-end verification.** Run from the repo root:
```bat
reproduce.bat
```
It performs: toolchain check → Go build+test → Rust build+test (94) →
**differential test** (`fc` of both interpreters on `prog.monkey`, prints
`IDENTICAL`) → **determinism** (3 runs) → LOC/AO → MS (`unsafe`, fns) → ID
(`clippy`) → TD (deps). Artifacts land in `repro_out\`. The differential program
is `prog.monkey` (inline comments only — see note below).

> **Note (a faithful Go limitation, preserved).** The lexer skips only *one*
> comment per token, so two *adjacent* comment-only lines tokenise the second
> `//` as `/` — in **both** Go and Rust (a shared, equivalent limitation).
> `prog.monkey` therefore uses inline comments only. This is distinct from the
> comment-at-EOF *bug* that was fixed.

Unix/macOS equivalents:
```bash
go test ./... && go build -o monkey_go .
cargo test --manifest-path rust/Cargo.toml
diff <(./monkey_go prog.monkey) <(./rust/target/debug/monkey prog.monkey) && echo IDENTICAL
cargo clippy --manifest-path rust/Cargo.toml --all-targets   # expect 0 warnings
```

## Appendix C — Readability review protocol (RS, manual)
1. Sample 6 functions stratified across modules (lexer, parser, eval, object).
2. Two independent reviewers rate each 1–5 (1 = unreadable, 5 = exemplary) on:
   naming, control-flow clarity, comment usefulness, idiomaticity.
3. Report mean per dimension and **Cohen's κ** for inter-rater agreement;
   κ ≥ 0.6 ⇒ acceptable agreement. Record disagreements as review actions.

## Appendix D — Supporting artifacts (in `rust/docs/`)
- `construct_mappability.md` — worked CMS examples (Direct/Adapted/Re-arch).
- `translation_notes.md` — full mapping, preserved quirks, improvements.
- `ast_report.html` — AST/dependency visualisation.
- `prompts.md` — the staged prompt protocol (treatment definition).
- `../../reproduce.bat`, `../../prog.monkey` — reproduction package.
