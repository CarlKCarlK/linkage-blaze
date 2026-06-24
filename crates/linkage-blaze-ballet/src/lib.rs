#![no_std]

#[cfg(test)]
mod tests {
    const DOF: usize = 132;
    const FRAME_COUNT: usize = 592;

    include!("ballet_frames_precomputed.rs");
    // The include above defines `pub static FRAMES: [[f32; DOF]; FRAME_COUNT]`.

    #[allow(long_running_const_eval)]
    const CONST_MOTION: linkage_blaze_core::bvh_parse::BvhMotion<132, 592> =
        linkage_blaze_core::bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");

    #[test]
    fn precomputed_matches_const_fn() {
        // CONST_MOTION is u16-quantized; allow up to 1 LSB of rounding error.
        const TOLERANCE: f32 = 1.0 / 65535.0;
        for frame_idx in 0..CONST_MOTION.frame_count() {
            let const_frame = CONST_MOTION.frame(frame_idx);
            let precomputed = &FRAMES[frame_idx];
            for ch in 0..CONST_MOTION.dof() {
                let diff = (const_frame[ch] - precomputed[ch]).abs();
                assert!(
                    diff <= TOLERANCE,
                    "frame {frame_idx} ch {ch}: diff {diff} exceeds {TOLERANCE}"
                );
            }
        }
    }
}
