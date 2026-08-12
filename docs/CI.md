# CI Pipeline

Multiplexer enforces a **strict, ordered gate chain** on every push to `main` and every pull request. Each stage must be green before the next runs, and **all must be green before merge**. This is the "no blind CI" guarantee: the same gates run locally before you push (see [Local dev loop](#local-dev-loop)).

The pipeline lives in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) and runs on a **Windows runner** (the project is Windows-first; the vendored `grok-build` builds on Windows). The Rust toolchain is pinned to **1.94.0**, matching `third_party/grok-build/rust-toolchain.toml` and the toolchain that produced `mutants.out/` and `target/llvm-cov/html`.

## The ordered gates

```
1. fmt            cargo fmt --check
2. clippy         cargo clippy --workspace -- -D warnings      (deny warnings)
3. unit+property  cargo test --workspace
4. mutation       cargo mutants, >=70% killed                  (cargo-mutants 27.1.0)
5. coverage       cargo llvm-cov, >=85% line / >=80% branch
6. integration/   cargo test --workspace --all-targets         (placeholder)
   component/e2e
```

**Order matters.** fmt and clippy fail fast (cheap); mutation and coverage run late (expensive). A formatting error never wastes a mutation run.

### Gate 4: mutation (cargo-mutants)

`cargo-mutants` introduces small faults ("mutants") into the code and re-runs the suite against each. A mutant the tests fail to catch is a **survived** mutant, and `cargo-mutants` exits non-zero when any survive, so a missed mutant fails the gate. The **70% kill floor is the merge bar** (decision D33); the bar may rise over time. Survivors are a code smell, not just a test gap: fix the test or the dead code, never silence a survivor.

The CI invocation uses `--in-place` to avoid the default copy-tree mode (which copies the whole tree, roughly 30 GB here) while still writing the same `mutants.out/` format. `.cargo/mutants.toml` restricts generation to `crates/**/*.rs` and excludes `third_party/**` and `spike/**`. The mutation step sets `CARGO_INCREMENTAL=0` so in-place rebuilds cannot reuse a stale incremental cache. The version is pinned to **27.1.0**, the version that produced the existing `mutants.out/` report.

### Gate 5: coverage (cargo-llvm-cov)

Enforces the plan/15 gates: **>=85% line** and **>=80% branch**. The HTML report is uploaded as a CI artifact. Coverage is a floor, not a target; the mutation gate is the stronger signal.

### Gate 6: integration / component / e2e (placeholder)

These suites do not exist yet. This step runs `cargo test --workspace --all-targets` so any integration tests (`tests/`) are exercised. Component (GPUI) and e2e gates will be added here as those suites are built, per `plan/15-testing-strategy.md` section 5. The full plan order (integration -> component -> e2e -> coverage) is restored once those suites land.

## Local dev loop

Run the same gates locally before pushing. Do not rely on CI to catch what you can catch locally.

```bash
# 1. fmt
cargo fmt --check

# 2. clippy (deny warnings)
cargo clippy --workspace -- -D warnings

# 3. unit + property
cargo test --workspace

# 4. mutation (>=70% killed; slow)
cargo install cargo-mutants --version 27.1.0 --locked
# .cargo/mutants.toml excludes third_party/** and spike/**
CARGO_INCREMENTAL=0 cargo mutants --in-place --timeout 30

# 5. coverage (>=85% line, >=80% branch; slow)
cargo install cargo-llvm-cov --locked
cargo llvm-cov --workspace --fail-under-lines 85 --fail-under-branches 80 --html --output-dir target/llvm-cov/html

# 6. integration / component / e2e (placeholder)
cargo test --workspace --all-targets
```

Quick sanity checks for a single crate (fast, no mutation/coverage):

```bash
cargo fmt --check
cargo clippy -p multiplexer-wire -- -D warnings
cargo test -p multiplexer-wire
```

## Adding the real integration / component / e2e gates

When the integration suite (`tests/`), GPUI component tests, and e2e tests are built, replace the Gate 6 placeholder with the real steps in the plan order:

```text
integration    cargo test --workspace --test '*'
component      (GPUI component + snapshot tests)
e2e            (real/headless app; merge gate: critical paths, nightly: full suite)
coverage       (runs last, after the full suite executes)
```

See `plan/15-testing-strategy.md` section 5 for the authoritative gate definitions and thresholds.