// Feature `const-parse`: parse pirouette.bvh at compile time via const fn (~8 s).
// Default (no feature): include a pre-generated snapshot; regenerate with `just generate-ballet`.

pub const BALLET_DOF: usize = 132;
pub const BALLET_FRAME_COUNT: usize = 592;

// ── const-parse path ─────────────────────────────────────────────────────────
//
// Channels 0-2: hip Xpos/Ypos/Zpos — range [-300, 300].
// Channels 3-131: joint rotations  — range [-720, 720].
// Values within ±0.01 of 0.5 after normalization snap to exactly 0.5,
// matching the runtime behavior of linkage-blaze-mocap's `snap_centered_default`.

#[cfg(feature = "const-parse")]
#[allow(long_running_const_eval)]
pub const BALLET_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] = {
    normalize(crate::bvh_parse::parse_bvh_motion_section::<BALLET_DOF, BALLET_FRAME_COUNT>(
        include_bytes!("../../linkage-blaze-mocap/samples/pirouette.bvh"),
    ))
};

#[cfg(feature = "const-parse")]
const POSITION_CHANNELS: usize = 3;
#[cfg(feature = "const-parse")]
const POSITION_LOW: f32 = -300.0;
#[cfg(feature = "const-parse")]
const POSITION_RANGE: f32 = 600.0;
#[cfg(feature = "const-parse")]
const ROTATION_LOW: f32 = -720.0;
#[cfg(feature = "const-parse")]
const ROTATION_RANGE: f32 = 1440.0;
#[cfg(feature = "const-parse")]
const SNAP_CENTER: f32 = 0.5;
#[cfg(feature = "const-parse")]
const SNAP_EPSILON: f32 = 0.01;

#[cfg(feature = "const-parse")]
const fn normalize(
    raw: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT],
) -> [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] {
    let mut out = [[0.0f32; BALLET_DOF]; BALLET_FRAME_COUNT];
    let mut frame = 0;
    while frame < BALLET_FRAME_COUNT {
        let mut ch = 0;
        while ch < BALLET_DOF {
            let v = raw[frame][ch];
            let norm = if ch < POSITION_CHANNELS {
                (v - POSITION_LOW) / POSITION_RANGE
            } else {
                (v - ROTATION_LOW) / ROTATION_RANGE
            };
            // Manual abs — f32::abs is not available in const fn.
            let diff = if norm > SNAP_CENTER {
                norm - SNAP_CENTER
            } else {
                SNAP_CENTER - norm
            };
            out[frame][ch] = if diff <= SNAP_EPSILON { SNAP_CENTER } else { norm };
            ch += 1;
        }
        frame += 1;
    }
    out
}

// ── pre-generated path ───────────────────────────────────────────────────────

#[cfg(not(feature = "const-parse"))]
include!("ballet_frames_precomputed.rs");
