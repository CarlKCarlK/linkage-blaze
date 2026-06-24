#![no_std]

#[cfg(test)]
mod tests {
    const BALLET_DOF: usize = 132;
    const BALLET_FRAME_COUNT: usize = 592;

    include!("ballet_frames_precomputed.rs");
    // The include above defines `pub static BALLET_FRAMES` for use below.

    #[allow(long_running_const_eval)]
    const CONST_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] = linkage_blaze_core::bvh_frames!(
        "../../linkage-blaze-mocap/samples/pirouette.bvh",
        BALLET_DOF,
        BALLET_FRAME_COUNT
    );

    #[test]
    fn precomputed_matches_const_fn() {
        assert_eq!(BALLET_FRAMES, CONST_FRAMES);
    }
}
