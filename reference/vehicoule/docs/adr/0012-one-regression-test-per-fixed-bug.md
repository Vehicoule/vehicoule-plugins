# 0012. Give every fixed bug a regression test named after its issue

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

The first app's queue and player state broke in ways no test caught, and some bugs came back after a fix
([ADR 0004](0004-verification.md)). A fix with no test that fails without it can be undone by any later change, and
nobody can tell which test guards which bug.

ripgrep (`BurntSushi/ripgrep` at 91f5d22) keeps one test per reported bug in `tests/regression.rs`, named after the
issue (`r16`, `r1159_invalid_flag`) with a `// See: https://github.com/BurntSushi/ripgrep/issues/16` line above it;
104 such tests at that commit. Reading the name gives the bug, and the issue gives the context.

The user agreed on 2026-11-07 to take this practice.

## Options

1. **One regression test per fixed bug, named after its issue, beside the code it covers.** The test lives where the
   other tests of that code live (the module's `#[cfg(test)]` module, or the crate's integration tests when the bug is
   only visible through the public API), so it is maintained with them. Costs one test per fix.
2. **One regression test per fixed bug, all in one file per crate, as ripgrep.** Easy to list, but the test sits far
   from the code it covers, and a crate whose bug needs private items cannot reach them from `tests/`.
3. **No rule.** Fixes may come without a test, as in the first app.

## Decision

Option 1. A pull request that fixes a bug adds a test that fails before the fix and passes after it:

- Its name starts with `regression_` and the issue number: `regression_42_removed_track_is_not_played`. A bug found
  with no issue gets an issue first, so the number exists.
- A comment above it links the issue: `// See: https://github.com/Vehicoule/Vehicoule/issues/42`.
- It checks what must happen and what must not (AGENTS.md rule 7), like every test.
- The pull request says that the test fails without the fix, and how that was checked.

A regression test is never deleted or weakened to get green (AGENTS.md rules 2 and 7); if the behavior it guards
changes on purpose, the pull request says so and links the issue.

## Consequences

- `cargo test regression_` runs every regression test; `cargo test regression_42` the one of issue 42.
- Mutation testing (`cargo-mutants`) and Kani still apply: a regression test does not replace a proof or a property
  test where those fit ([ADR 0004](0004-verification.md)); when a bug breaks an invariant a Kani proof or a property
  test should have covered, the fix adds or widens that proof too.
- Reviews check that a fix comes with its regression test.
