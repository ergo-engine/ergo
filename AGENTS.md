# AGENTS.md — Final v1 Production Hardening Pass

This file defines how agents should work in this repository during the
final production pass for v1.

The goal is not merely "make it pass" or "make it cleaner."

The goal is:

- make every important file understandable
- make every important type and boundary safe in the semantic sense
- make ownership and enforcement obvious
- reduce the odds that a future team decision forces an avoidable return
  to the same file

North star:

> Does this make sense? Do I really fully understand this? As it stands,
> are there any future development team decisions that would otherwise
> make me come back here and change something?

If the answers are not:

- `yes`
- `yes`
- `no`

then the file is not done.

---

## 1. Read This First

Before changing a file, ground yourself in the authoritative docs for
its layer.

Read in this order when needed:

1. `docs/INDEX.md`
2. `docs/system/kernel.md`
3. `docs/system/current-architecture.md`
4. `docs/system/kernel-prod-separation.md`
5. The relevant file in `docs/invariants/`
6. The relevant primitive/authoring/contract doc for the surface you are touching

Authority rule:

- `FROZEN` docs beat code
- `STABLE` docs beat ad hoc implementation drift
- `CANONICAL` docs describe the current intended system
- crate-local READMEs and comments are informative, not authoritative

If code conflicts with higher-authority docs, assume the code is wrong
until proven otherwise.

Do not silently rewrite docs to match confusing code.

---

## 2. Scope of This Pass

This pass is primarily for:

- `crates/kernel/*`
- `crates/prod/*`
- `crates/shared/*`
- `tools/*` when they enforce or protect architecture/boundaries
- important top-level project files when they shape production behavior

This pass is not primarily for:

- `target/*`
- generated artifacts
- lockfiles unless dependency changes require it
- fixture data or snapshot-like files unless they are normative contract fixtures

For fixture/test/support files, clarity still matters, but they do not
carry the same burden as production/kernel files.

---

## 3. Hardening Posture

This is the final production pass for v1.

Bias toward:

- explicitness over cleverness
- enforcement over convention
- local clarity over hidden coupling
- stable boundaries over opportunistic reuse
- small, high-confidence refactors over large speculative rewrites

Do not use this pass to:

- add new semantics casually
- widen public API surface without clear need
- invent a second source of truth in file headers
- move kernel meaning into prod
- move prod orchestration into kernel

Headers and local comments should explain the file's role in the
existing system. They must not become alternate specs that drift from
the canonical docs.

---

## 4. Required Work For Each File

For each in-scope file, ensure all of the following are true.

### A. Add or improve a file header

Every important source file should begin with a short header explaining
what it is and why it exists.

Use the local file's comment style:

- Rust source: `//!`
- shell/Python: leading file comment block
- Markdown/docs: only when the document lacks a clear top-level purpose

The header should cover:

- file name or module identity
- what the file does
- what layer owns it
- what it connects to
- what it explicitly does not own or enforce
- the most important safety/invariant notes

Recommended Rust header shape:

```rust
//! <file or module name>
//!
//! Purpose:
//! - <what this file is for>
//!
//! Owns:
//! - <responsibilities enforced here>
//!
//! Does not own:
//! - <things enforced elsewhere>
//!
//! Connects to:
//! - <main upstream/downstream collaborators>
//!
//! Safety notes:
//! - <key invariants, misuse risks, or ordering assumptions>
```

Keep headers short and truthful. If the explanation is long, the design
likely still needs work.

### B. Move non-essential tests out of production/kernel files

Production and kernel files should not be crowded by scenario-heavy or
fixture-heavy tests.

Rust module test placement convention for this repo:

- Treat this as the default out-of-line unit test pattern:
  - keep the production module root as `foo.rs`
  - keep `#[cfg(test)] mod tests;` in `foo.rs`
  - move the test body to `foo/tests.rs` or `foo/tests/mod.rs`
  - when the tests need further structure, split them under
    `foo/tests/*.rs`
  - inside the test module root, import the parent with `use super::*;`
- Do not convert `foo.rs` to `foo/mod.rs` merely to move tests out of
  the production file.
- Convert `foo.rs` to `foo/mod.rs` only when the production module
  itself needs production submodules and the directory-module form makes
  the production ownership clearer.
- Use crate-level `tests/` only for true integration tests that should
  compile against the crate from the outside rather than as child
  modules with private access.
- Keep tiny local invariant tests inline only when they are the
  smallest, clearest way to protect a private helper or local semantic
  contract.

Required shape when extracting inline unit tests from `foo.rs`:

```rust
// foo.rs
#[cfg(test)]
mod tests;
```

- Preferred filesystem layouts:
  - `foo.rs` + `foo/tests.rs`
  - `foo.rs` + `foo/tests/mod.rs` + `foo/tests/<group>.rs`
- Non-preferred layout for test extraction alone:
  - `foo/mod.rs` only because tests moved

This is a hard repo convention. Follow it unless the production module
shape itself gives a stronger reason to use `foo/mod.rs`.

Move tests out when they are:

- integration-shaped
- replay/protocol/fixture-heavy
- mostly exercising external behavior instead of local private logic
- large enough that they obscure the production code

It is acceptable to keep tests inline when they are:

- tightly coupled to a private helper
- the smallest and clearest place to express a local invariant
- meaningfully easier to understand next to the code they protect

Default rule:

- keep local invariant tests near private helpers
- move broader behavior tests into dedicated test files

When moving tests, preserve coverage and keep names stable when possible.

### C. Audit every struct, enum, trait, and important type alias for safety

In this repo, "safe" means more than Rust memory safety.

Ask of every important type:

- How is it constructed?
- Can it exist in an invalid state?
- Who validates it?
- Who is allowed to mutate it?
- Who consumes it?
- What assumptions do callers have to satisfy?
- What happens if the type is extended later?
- Does the name accurately signal its role and limits?

Check especially:

- public fields
- `Default` impls
- `Clone`, `Copy`, `Eq`, `Ord`, `Hash` semantics
- serde defaults, aliases, renames, and compatibility behavior
- trait object surfaces and blanket impls
- hidden ordering assumptions
- panic/`unwrap`/`expect` usage
- non-obvious lifetime or ownership coupling
- kernel determinism requirements
- host/kernel boundary ownership

If a type is only safe because "callers know not to do that," it is not
safe enough for this pass unless that contract is made explicit and the
layer ownership is correct.

### D. Make the larger shape understandable

Every important file should answer:

- What is this file's job in the larger architecture?
- What calls into it?
- What does it call out to?
- What layer owns the meaning here?
- Which docs or invariants govern it?
- What would break or become ambiguous if this file changed?

If you cannot explain the larger shape in a few sentences, keep working.

### E. Make enforcement boundaries explicit

For each file, be clear about:

- what this file enforces
- what it assumes
- what it deliberately does not enforce
- whether it is semantic authority, orchestration glue, transport/decode,
  test support, or tooling

This matters especially for:

- kernel vs prod boundaries
- host vs loader ownership
- adapter contract vs runtime behavior
- boundary channel code vs host dispatch logic

### F. Identify technical debt and refactor pressure

Do not stop at "it works."

Identify whether the file still contains:

- ambiguous ownership
- duplicated enforcement
- misleading names
- hidden semantic coupling
- broad modules doing too many jobs
- test scaffolding mixed into production logic
- compatibility behavior that is accidental rather than explicit
- boundary bleed between kernel and prod

When debt is found, either:

- fix it now if the refactor is local and high-confidence, or
- record the risk clearly in the review output and explain why it is not
  being fixed in this pass

### G. Research downstream imports and breakage before refactoring

Before moving code, splitting modules, renaming types/functions, or
changing visibility, do a concrete downstream usage audit first.

This is a hard requirement. Do not refactor first and "see what breaks"
afterward.

For every candidate move or API-shape change, explicitly determine:

- which files import or reference the item today
- which crates depend on the current path, visibility, or module shape
- whether the item is part of a public or cross-crate surface
- whether tests, docs, fixtures, or serialized names rely on the current
  spelling or location
- whether the proposed refactor preserves those paths through
  re-exports, or would require downstream edits

Minimum required audit steps:

- search the workspace for downstream imports and symbol references
  before editing
- identify whether callers are same-file, same-module, same-crate, or
  cross-crate
- list the exact paths that must remain stable unless an explicit API
  change is approved
- prefer preserving existing public paths through re-exports rather than
  forcing broad call-site churn
- escalate before proceeding if the refactor appears to require a public
  API move, a semantic rename, or a broad compatibility sweep

Required default posture:

- preserve public import paths unless there is a stronger architectural
  reason not to
- preserve visibility contracts unless tightening them is clearly safe
  and all downstream usage has been checked
- do not move kernel-owned seams into prod just because the code would
  "look cleaner"
- do not rely on compiler errors alone as the dependency audit

When presenting a refactor proposal, state all of the following
concretely:

- what is moving
- what is not moving
- which downstream imports were checked
- whether existing paths will stay valid
- what verification will prove the refactor did not break dependents

---

## 5. What "Safe" Means Here

For this pass, a file is not "safe" just because it compiles and tests pass.

A file is safe when:

- invalid states are prevented, rejected, or clearly isolated
- enforcement happens in the correct layer
- names match responsibility
- surprising behavior is either removed or documented
- extension points fail predictably
- persisted/serialized surfaces are treated intentionally
- deterministic behavior is preserved where required
- callers are not forced to rely on tribal knowledge

For kernel files, "safe" also means:

- no semantic drift from frozen meaning
- no hidden defaults or coercions
- no product-specific logic
- deterministic behavior remains obvious and defendable

For prod files, "safe" also means:

- no redefinition of kernel meaning
- orchestration ownership remains in host
- loader stays transport/decode/discovery rather than semantic authority
- client layers remain thin over host

---

## 6. Definition of Done For A File

A file is done only when all of the following are true:

1. Its purpose is obvious from the header and the code shape.
2. Its place in the larger architecture is understandable.
3. Its important types and traits are reviewed for misuse, invalid
   states, and extension risk.
4. Non-essential tests are moved out of the production path.
5. Enforcement vs assumption is clear.
6. Technical debt is either reduced or explicitly called out.
7. A future maintainer can answer "why does this exist, why here, and
   why is this safe?" without reconstructing half the repo.

If any of those are still shaky, do more work.

---

## 7. Escalate Instead Of Guessing

Stop and escalate when:

- code and authoritative docs disagree in a non-mechanical way
- fixing the file would require a semantic decision rather than a clarification
- a boundary line between kernel and prod becomes unclear
- a public API change seems necessary
- a persisted format compatibility decision is implicated
- the right fix spans many files and the local change would only hide the problem

Do not paper over structural confusion with comments alone.

---

## 8. Verification Expectations

After hardening a file:

- run the most relevant tests for the touched crate/module
- run broader verification when the change affects boundaries or shared behavior
- when code moved or public paths were involved, run focused verification
  for each downstream crate or module that imports the moved surface
- confirm moved tests still execute from their new location
- confirm docs/comments still match actual behavior

A header that explains the wrong thing is worse than no header.

---

## 9. Review Questions To Apply Repeatedly

Ask these questions for every file:

1. Does this file make immediate sense?
2. Do I fully understand how it is used?
3. Do I understand what larger shape it participates in?
4. Is every important type here safe against plausible misuse?
5. Is it obvious what this file enforces and what it does not?
6. Are the tests in the right place?
7. Is there any likely future decision that would force an avoidable revisit?

If the answers are not strongly:

- `yes`
- `yes`
- `yes`
- `yes`
- `yes`
- `yes`
- `no`

then continue hardening.

---

## 10. Practical Output Expectations

When reporting work on a file, cover:

- header added or improved
- tests moved or intentionally kept in place
- safety/invariant issues found
- larger-shape clarification added
- enforcement boundaries clarified
- technical debt found
- downstream imports checked and whether public paths changed
- verification run

Concise is good. Vague is not.
