# `linkage-blaze-utils` has moved

This package has moved. Use [`linkage-blaze`](https://crates.io/crates/linkage-blaze)
instead. Linkage Blaze now provides its allocation-free embedded API, optional
allocation support, BVH conversion, and the `bvh-to-lb` command from one crate.

BVH users should enable the `bvh` feature and install the converter with:

```bash
cargo install linkage-blaze --features bvh --bin bvh-to-lb
```

Helpful links:

- [Crates.io package](https://crates.io/crates/linkage-blaze)
- [API documentation](https://docs.rs/linkage-blaze)
- [GitHub repository](https://github.com/CarlKCarlK/linkage-blaze)
- [Repository README](https://github.com/CarlKCarlK/linkage-blaze#readme)

This package is a documentation tombstone. It does not provide the former API
and will not receive further releases.
