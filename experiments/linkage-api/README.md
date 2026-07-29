# Linkage API Experiment

This is a disposable, independent Cargo project for rapidly testing Linkage
Blaze API shapes. It does not depend on `linkage-blaze-core` and is not a
member of the repository's production workspace.

Run it directly:

```text
cargo test --manifest-path experiments/linkage-api/Cargo.toml
```

The model intentionally contains only:

- fixed step storage;
- a view that erases step capacity;
- a small fluent operation vocabulary;
- const promotion of fluent linkage expressions;
- view-in/view-out `linkage_combine!`.

The current candidate call sites live in `tests/api.rs`. Change those first
when trying a different API shape, then make the smallest implementation
change needed to compile them.

This project tests API spelling and stable Rust feasibility. It is not a
second production implementation: parameter and mark storage, evaluation,
rendering, diagnostics, and the complete fluent vocabulary remain outside its
scope.

