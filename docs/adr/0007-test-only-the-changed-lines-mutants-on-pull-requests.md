# 0007. Test only the changed lines' mutants on pull requests

- Status: Accepted
- Date: 2026-11-08
- Deciders: the user

## Context

- ADR 0006 runs `cargo mutants` on the whole workspace for every pull request: a mutant that survives fails the
  check.
- Runners are what Vehicoule's four repositories wait for. On 2026-11-08 jobs waited up to an hour for a runner, and
  every job of a pull request waits for its own.
- Here the whole set takes 1 to 4 minutes on one runner (3.9 minutes on a pull request's run of 2026-11-08). It costs
  far more in Vehicoule and nsh-m3e, about 27 and 160 runner-minutes per pull request, and the four repositories keep
  one rule.
- cargo-mutants 27.1.0's `--in-diff FILE` keeps only the mutants that overlap the lines a diff changes, after the other
  filters and before sharding. A diff that is empty, changes no Rust source or touches no mutant tests nothing and
  succeeds; a diff that does not match the source tree fails (its book, "Testing code changed in a diff", and
  `DiffFilterError::exit_code` in `src/in_diff.rs`, at v27.1.0). Its book warns that a change can leave code outside
  the diff less well tested, and that a diff touching only tests runs no mutant.

## Options

1. Keep the whole set on every pull request. Every pull request checks every mutant, at the cost above, whatever it
   changes.
2. On a pull request, test only the mutants in the lines it changes; on `main`, after each merge, test the whole set.
   A pull request that weakens the tests of code it does not change is caught on `main` after its merge, not before.
3. Test only the changed lines everywhere. Nothing would ever check the whole set again.

## Decision

Option 2, chosen by the user on 2026-11-08 for the four repositories (Vehicoule, vehicoule-plugins,
vehicoule-plugins-extra, nsh-m3e).

## Consequences

- On a pull request, the mutants job gives cargo-mutants `--in-diff` with `git diff HEAD^1 HEAD`: GitHub's merge commit
  against its first parent, the base branch, which is what the pull request changes. A push to `main` and a run by hand
  test every mutant, as before.
- A mutants run that fails on `main` is fixed at once, like a failing test: a merged pull request weakened a test
  outside its diff.
