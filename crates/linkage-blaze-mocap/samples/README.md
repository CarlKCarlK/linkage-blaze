# Motion Capture Samples

Expected BVH sample filenames:

- `pirouette.bvh` from the Three.js BVH loader sample data
- `pirouette.lb.rs` generated from `pirouette.bvh`

These are intentionally gitignored for now. Download or copy the BVH sample,
then place it here with the name above. Generate the `.lb.rs` file with:

```bash
cargo run -p linkage-blaze-mocap --example bvh_to_lb_rs -- \
    crates/linkage-blaze-mocap/samples/pirouette.bvh \
    crates/linkage-blaze-mocap/samples/pirouette.lb.rs
```

## Licensing

**Code:** MIT OR Apache-2.0 (same as the rest of this workspace).

**BVH sample data:** third-party CMU/CGSpeed mocap data; not relicensed by us.
