#![no_std]

#[cfg(test)]
mod tests {
    const DOF: usize = 132;
    const SAMPLE_COUNT: usize = 592;

    include!("ballet_frames_precomputed.rs");
    // The include above defines `pub static SAMPLES: [[f32; DOF]; SAMPLE_COUNT]`.

    #[allow(long_running_const_eval)]
    const CONST_MOTION: linkage_blaze_core::bvh_parse::BvhMotion<132, 592> =
        linkage_blaze_core::bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");

    #[test]
    fn precomputed_matches_const_fn() {
        // CONST_MOTION is u16-quantized; allow up to 1 LSB of rounding error.
        const TOLERANCE: f32 = 1.0 / 65535.0;
        for sample_idx in 0..CONST_MOTION.sample_count() {
            let const_sample = CONST_MOTION.sample(sample_idx);
            let precomputed = &SAMPLES[sample_idx];
            for ch in 0..CONST_MOTION.dof() {
                let diff = (const_sample[ch] - precomputed[ch]).abs();
                assert!(
                    diff <= TOLERANCE,
                    "sample {sample_idx} ch {ch}: diff {diff} exceeds {TOLERANCE}"
                );
            }
        }
    }
}
