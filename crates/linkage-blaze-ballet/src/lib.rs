#![no_std]

#[cfg(test)]
mod tests {
    const BALLET_DOF: usize = 132;
    const BALLET_FRAME_COUNT: usize = 592;

    include!("ballet_frames_precomputed.rs");
    // The include above defines `pub static BALLET_FRAMES: [[f32; DOF]; FRAMES]`.

    #[allow(long_running_const_eval)]
    const CONST_FRAMES: linkage_blaze_core::bvh_parse::BvhMotion<
        BALLET_DOF,
        BALLET_FRAME_COUNT,
    > = linkage_blaze_core::bvh_frames!(
        "../../linkage-blaze-mocap/samples/pirouette.bvh",
        BALLET_DOF,
        BALLET_FRAME_COUNT
    );

    #[test]
    fn precomputed_matches_const_fn() {
        // CONST_FRAMES is u16-quantized; allow up to 1 LSB of rounding error.
        const TOLERANCE: f32 = 1.0 / 65535.0;
        for frame_idx in 0..BALLET_FRAME_COUNT {
            let const_frame = CONST_FRAMES.frame(frame_idx);
            let precomputed = &BALLET_FRAMES[frame_idx];
            for ch in 0..BALLET_DOF {
                let diff = (const_frame[ch] - precomputed[ch]).abs();
                assert!(
                    diff <= TOLERANCE,
                    "frame {frame_idx} ch {ch}: diff {diff} exceeds {TOLERANCE}"
                );
            }
        }
    }
}
