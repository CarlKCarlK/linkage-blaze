#![no_std]

pub mod bvh_parse;

#[cfg(test)]
mod tests {
    const BALLET_DOF: usize = 132;
    const BALLET_FRAME_COUNT: usize = 592;

    include!("ballet_frames_precomputed.rs");
    // The include above defines `pub static BALLET_FRAMES` for use below.

    const BVH_BYTES: &[u8] =
        include_bytes!("../../linkage-blaze-mocap/samples/pirouette.bvh");

    #[allow(long_running_const_eval)]
    const RAW_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] =
        crate::bvh_parse::parse_bvh_motion_section::<BALLET_DOF, BALLET_FRAME_COUNT>(BVH_BYTES);

    const CHANNEL_IS_POSITION: [bool; BALLET_DOF] =
        crate::bvh_parse::parse_bvh_channel_is_position::<BALLET_DOF>(BVH_BYTES);

    #[allow(long_running_const_eval)]
    const CONST_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] =
        crate::bvh_parse::normalize_bvh_motion::<BALLET_DOF, BALLET_FRAME_COUNT>(
            RAW_FRAMES,
            CHANNEL_IS_POSITION,
            crate::bvh_parse::BvhNormalizePolicy::LINKAGE_BLAZE,
        );

    #[test]
    fn precomputed_matches_const_fn() {
        assert_eq!(BALLET_FRAMES, CONST_FRAMES);
    }
}
