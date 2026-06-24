//! Const-fn parser for the BVH motion section.
//!
//! Parses the numeric subset used in BVH motion files:
//!
//!   `['-'|'+'] (digits ['.' digits?] | '.' digits) [('e'|'E') ['-'|'+'] digits]`
//!
//! Leading and trailing decimal points are accepted (`.5` and `42.`).
//! At least one digit must appear somewhere in the mantissa.
//!
//! Returns raw `f32` values exactly as they appear in the file — no
//! normalization or channel-mapping is applied here.  Callers are responsible
//! for interpreting the channel order and value ranges.
//!
//! # Algorithm
//!
//! For each number:
//! 1. Accumulate up to `MAX_MANTISSA_DIGITS` significant digits into a `u64`
//!    mantissa (panics if more than 18 significant digits are present).
//! 2. Track `frac_digits` — how many digits follow the decimal point.
//! 3. After an optional `e`/`E` exponent, the effective power-of-ten is
//!    `e_exp − frac_digits`.
//! 4. `value = mantissa × 10^decimal_exp` via repeated multiply/divide.
//!
//! This is not correctly-rounded IEEE 754 parsing (unlike `str::parse::<f32>`)
//! but it is deterministic, allocation-free, and accurate enough for BVH motion
//! visualization.  Validated against `str::parse::<f32>` for representative BVH
//! values with relative tolerance 1e-5.

// todo000 article: Parsing a 764 KB BVH file in a Rust const fn takes ~8 s.
// Is that acceptable for an embedded project?  Weigh against the alternative:
// a `just generate-ballet` code-generation step checked in as
// `ballet_frames_precomputed.rs`.  Neither answer is clearly right.

const MAX_MANTISSA_DIGITS: usize = 18;

// ── public API ───────────────────────────────────────────────────────────────

/// Parse and normalize a BVH file embedded at compile time.
///
/// The path is resolved relative to the file that invokes the macro, exactly
/// like a bare `include_bytes!`.  `DOF` and `FRAMES` must match the channel
/// count and frame count in the file; a mismatch panics at compile time with a
/// descriptive message.
///
/// # Example
///
/// ```rust,ignore
/// #[allow(long_running_const_eval)]
/// const FRAMES: BvhMotion<132, 592> =
///     linkage_blaze_core::bvh_frames!("path/to/motion.bvh", 132, 592);
/// ```
#[macro_export]
macro_rules! bvh_frames {
    ($file:expr, $dof:expr, $frames:expr) => {
        $crate::bvh_parse::parse_and_normalize_bvh_motion::<$dof, $frames>(include_bytes!(
            $file
        ))
    };
}

/// u16 value that decodes to exactly 0.5.
///
/// BVH normalization snaps near-center values to exactly 0.5 (the linkage
/// parameter center/default). This constant preserves that exact 0.5 through
/// the u16 round-trip without floating-point rounding error.
pub const PARAM_CENTER_U16: u16 = 32768;

/// Encode a normalized `[0.0, 1.0]` parameter value as a `u16`.
///
/// Panics at compile time if `v` is outside `[0.0, 1.0]`.
/// Exactly 0.5 maps to [`PARAM_CENTER_U16`] and back without error.
pub const fn norm_to_u16(v: f32) -> u16 {
    assert!(v >= 0.0, "normalized value is below 0.0");
    assert!(v <= 1.0, "normalized value is above 1.0");
    if v == 0.5 {
        PARAM_CENTER_U16
    } else {
        (v * 65535.0 + 0.5) as u16
    }
}

/// Decode a `u16` back to a normalized `[0.0, 1.0]` parameter value.
///
/// [`PARAM_CENTER_U16`] decodes to exactly 0.5.
pub const fn u16_to_norm(x: u16) -> f32 {
    if x == PARAM_CENTER_U16 {
        0.5
    } else {
        x as f32 * (1.0 / 65535.0)
    }
}

/// Normalized BVH motion data stored as quantized `u16` values.
///
/// Each `f32` parameter in `[0.0, 1.0]` is encoded as a `u16` in `[0, 65535]`,
/// halving memory use. Decode one frame at a time with [`frame`](BvhMotion::frame)
/// or [`frame_into`](BvhMotion::frame_into).
pub struct BvhMotion<const DOF: usize, const FRAMES: usize> {
    frames: [[u16; DOF]; FRAMES],
}

impl<const DOF: usize, const FRAMES: usize> BvhMotion<DOF, FRAMES> {
    /// Construct from a pre-quantized `u16` frame array.
    pub const fn new(frames: [[u16; DOF]; FRAMES]) -> Self {
        Self { frames }
    }

    pub(crate) const fn from_normalized(f32_frames: [[f32; DOF]; FRAMES]) -> Self {
        let mut data = [[0u16; DOF]; FRAMES];
        let mut frame = 0;
        while frame < FRAMES {
            let mut ch = 0;
            while ch < DOF {
                data[frame][ch] = norm_to_u16(f32_frames[frame][ch]);
                ch += 1;
            }
            frame += 1;
        }
        Self { frames: data }
    }

    /// Return the number of frames.
    pub const fn frame_count(&self) -> usize {
        FRAMES
    }

    /// Decode one frame into a stack-allocated `[f32; DOF]` array.
    pub fn frame(&self, frame_index: usize) -> [f32; DOF] {
        let mut out = [0.0f32; DOF];
        self.frame_into(frame_index, &mut out);
        out
    }

    /// Decode one frame into an existing buffer, avoiding a local array.
    ///
    /// Preferred on embedded targets where minimizing stack pressure matters:
    ///
    /// ```rust,ignore
    /// let mut params = [0.0f32; DOF];
    /// for i in 0..motion.frame_count() {
    ///     motion.frame_into(i, &mut params);
    ///     // use params ...
    /// }
    /// ```
    pub fn frame_into(&self, frame_index: usize, out: &mut [f32; DOF]) {
        let packed = &self.frames[frame_index];
        let mut i = 0;
        while i < DOF {
            out[i] = u16_to_norm(packed[i]);
            i += 1;
        }
    }
}

/// Parse the MOTION section of a BVH file into a `FRAMES × DOF` array of raw
/// `f32` values.
///
/// `bytes` is the full BVH file.  `DOF` and `FRAMES` must exactly match the
/// channel count and frame count in the file; the parser asserts on mismatch
/// and panics at compile time if either is wrong.
pub const fn parse_bvh_motion_section<const DOF: usize, const FRAMES: usize>(
    bytes: &[u8],
) -> [[f32; DOF]; FRAMES] {
    let mut i = find_after(bytes, 0, b"MOTION");

    // "Frames:\t<count>\n"
    i = find_after(bytes, i, b"Frames:");
    i = skip_inline_whitespace(bytes, i);
    let (frame_count, next) = parse_uint(bytes, i);
    assert!(frame_count == FRAMES, "BVH Frames count does not match FRAMES");
    i = skip_to_next_line(bytes, next);

    // "Frame Time:\t<value>\n" — parse to confirm structure, discard value.
    i = find_after(bytes, i, b"Frame Time:");
    let (_, next) = parse_f32(bytes, skip_inline_whitespace(bytes, i));
    i = skip_to_next_line(bytes, next);

    let mut out = [[0.0f32; DOF]; FRAMES];
    let mut frame = 0;
    while frame < FRAMES {
        let mut ch = 0;
        while ch < DOF {
            i = skip_whitespace(bytes, i);
            let (value, next_i) = parse_f32(bytes, i);
            i = next_i;
            out[frame][ch] = value;
            ch += 1;
        }
        frame += 1;
    }

    // Reject trailing non-whitespace: catches DOF too small.
    let end = skip_whitespace(bytes, i);
    assert!(end == bytes.len(), "BVH: extra data after expected motion table");

    out
}

// ── float parser ─────────────────────────────────────────────────────────────

pub const fn parse_f32(bytes: &[u8], start: usize) -> (f32, usize) {
    let mut i = start;

    let negative = i < bytes.len() && bytes[i] == b'-';
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        i += 1;
    }

    let mut mantissa: u64 = 0;
    let mut sig_digits: usize = 0;
    let mut frac_digits: i32 = 0;
    let mut after_dot = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            assert!(!after_dot, "parse_f32: two decimal points in one number");
            after_dot = true;
            i += 1;
        } else if b >= b'0' && b <= b'9' {
            assert!(
                sig_digits < MAX_MANTISSA_DIGITS,
                "parse_f32: too many significant digits (max 18)"
            );
            mantissa = mantissa * 10 + (b - b'0') as u64;
            sig_digits += 1;
            if after_dot {
                frac_digits += 1;
            }
            i += 1;
        } else {
            break;
        }
    }

    assert!(sig_digits > 0, "parse_f32: expected at least one digit");

    let mut exp10: i32 = -frac_digits;

    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        let exp_neg = i < bytes.len() && bytes[i] == b'-';
        if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
            i += 1;
        }
        let mut exp_val: i32 = 0;
        let mut exp_digit_count: usize = 0;
        while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
            exp_val = exp_val * 10 + (bytes[i] - b'0') as i32;
            i += 1;
            exp_digit_count += 1;
        }
        assert!(exp_digit_count > 0, "parse_f32: exponent has no digits");
        if exp_neg {
            exp10 -= exp_val;
        } else {
            exp10 += exp_val;
        }
    }

    assert!(
        exp10 >= -100 && exp10 <= 100,
        "parse_f32: exponent out of supported range [-100, 100]"
    );

    // Scale in f64 then cast: eliminates the ~1-ULP rounding error that
    // accumulates when repeated ×10 / ÷10 steps are done in f32.
    let mut value = mantissa as f64;
    value = scale_pow10_f64(value, exp10);
    if negative {
        value = -value;
    }

    (value as f32, i)
}

pub const fn parse_uint(bytes: &[u8], start: usize) -> (usize, usize) {
    let mut i = start;
    let mut value: usize = 0;
    let mut digit_count: usize = 0;
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        value = value * 10 + (bytes[i] - b'0') as usize;
        i += 1;
        digit_count += 1;
    }
    assert!(digit_count > 0, "parse_uint: expected at least one digit");
    (value, i)
}

pub const fn scale_pow10(mut value: f32, mut exp: i32) -> f32 {
    while exp > 0 {
        value *= 10.0;
        exp -= 1;
    }
    while exp < 0 {
        value /= 10.0;
        exp += 1;
    }
    value
}

pub const fn scale_pow10_f64(mut value: f64, mut exp: i32) -> f64 {
    while exp > 0 {
        value *= 10.0;
        exp -= 1;
    }
    while exp < 0 {
        value /= 10.0;
        exp += 1;
    }
    value
}

/// Parse and normalize a BVH file's motion section in one step.
pub const fn parse_and_normalize_bvh_motion<const DOF: usize, const FRAMES: usize>(
    bytes: &[u8],
) -> BvhMotion<DOF, FRAMES> {
    let raw = parse_bvh_motion_section::<DOF, FRAMES>(bytes);
    let channel_is_position = parse_bvh_channel_is_position::<DOF>(bytes);
    let normalized =
        normalize_bvh_motion::<DOF, FRAMES>(raw, channel_is_position, BvhNormalizePolicy::LINKAGE_BLAZE);
    BvhMotion::from_normalized(normalized)
}

// ── normalization policy and helper ──────────────────────────────────────────

/// Linkage Blaze parameter-encoding policy for BVH channels.
///
/// These ranges are **not** extracted from the BVH file — BVH has no concept
/// of a valid-range declaration.  They are a Linkage Blaze design choice:
/// each parameter is stored as a `[0, 1]` float mapped from the physical range
/// below.  Keep them here, not buried inside the parser.
pub struct BvhNormalizePolicy {
    pub position_low: f32,
    pub position_high: f32,
    pub rotation_low: f32,
    pub rotation_high: f32,
    /// Normalized value to snap toward (typically 0.5 = centered).
    pub snap_center: f32,
    /// Half-width of the snap band around `snap_center`.
    pub snap_epsilon: f32,
}

impl BvhNormalizePolicy {
    /// Default Linkage Blaze policy: positions ±300, rotations ±720°.
    pub const LINKAGE_BLAZE: Self = Self {
        position_low: -300.0,
        position_high: 300.0,
        rotation_low: -720.0,
        rotation_high: 720.0,
        snap_center: 0.5,
        snap_epsilon: 0.01,
    };
}

/// Normalize a raw BVH motion table into `[0, 1]` Linkage parameters.
///
/// - `raw` — output of [`parse_bvh_motion_section`]
/// - `is_position` — output of [`parse_bvh_channel_is_position`]
/// - `policy` — Linkage Blaze parameter-range and snap policy
pub const fn normalize_bvh_motion<const DOF: usize, const FRAMES: usize>(
    raw: [[f32; DOF]; FRAMES],
    is_position: [bool; DOF],
    policy: BvhNormalizePolicy,
) -> [[f32; DOF]; FRAMES] {
    let pos_range = policy.position_high - policy.position_low;
    let rot_range = policy.rotation_high - policy.rotation_low;

    let mut out = [[0.0f32; DOF]; FRAMES];
    let mut frame = 0;
    while frame < FRAMES {
        let mut ch = 0;
        while ch < DOF {
            let v = raw[frame][ch];
            if is_position[ch] {
                assert!(
                    v >= policy.position_low,
                    "BVH position channel value is below Linkage Blaze normalization range"
                );
                assert!(
                    v <= policy.position_high,
                    "BVH position channel value is above Linkage Blaze normalization range"
                );
            } else {
                assert!(
                    v >= policy.rotation_low,
                    "BVH rotation channel value is below Linkage Blaze normalization range"
                );
                assert!(
                    v <= policy.rotation_high,
                    "BVH rotation channel value is above Linkage Blaze normalization range"
                );
            }
            let (low, range) = if is_position[ch] {
                (policy.position_low, pos_range)
            } else {
                (policy.rotation_low, rot_range)
            };
            let norm = (v - low) / range;
            out[frame][ch] = if (norm - policy.snap_center).abs() <= policy.snap_epsilon {
                policy.snap_center
            } else {
                norm
            };
            ch += 1;
        }
        frame += 1;
    }
    out
}

// ── channel-type scanner ──────────────────────────────────────────────────────

/// Scan the BVH hierarchy section and return which of the `DOF` channels are
/// position channels (`true`) versus rotation channels (`false`).
///
/// Reads every `CHANNELS N <type>...` line before the `MOTION` keyword, in
/// order.  Panics if the total channel count does not equal `DOF`.
pub const fn parse_bvh_channel_is_position<const DOF: usize>(bytes: &[u8]) -> [bool; DOF] {
    let hierarchy_end = find_motion_offset(bytes);

    let mut result = [false; DOF];
    let mut i = 0;
    let mut ch_index = 0;

    while i < hierarchy_end {
        if bytes_match(bytes, i, b"CHANNELS") {
            i += 8;
            i = skip_whitespace(bytes, i);
            let (count, next) = parse_uint(bytes, i);
            i = next;
            let mut c = 0;
            while c < count {
                i = skip_whitespace(bytes, i);
                let is_pos = bytes_match(bytes, i, b"Xposition")
                    || bytes_match(bytes, i, b"Yposition")
                    || bytes_match(bytes, i, b"Zposition");
                i = skip_token(bytes, i);
                assert!(ch_index < DOF, "BVH: more channels in file than DOF");
                result[ch_index] = is_pos;
                ch_index += 1;
                c += 1;
            }
        } else {
            i += 1;
        }
    }

    assert!(ch_index == DOF, "BVH: channel count does not match DOF");
    result
}

/// Return the byte offset of the `M` in `MOTION`, or `bytes.len()` if absent.
const fn find_motion_offset(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i + 6 <= bytes.len() {
        if bytes_match(bytes, i, b"MOTION") {
            return i;
        }
        i += 1;
    }
    bytes.len()
}

/// Skip forward past the current non-whitespace token.
pub const fn skip_token(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\r' | b'\n' => break,
            _ => i += 1,
        }
    }
    i
}

// ── byte-stream helpers ───────────────────────────────────────────────────────

/// Scan forward from `start` for `needle`; return the index just after it.
/// Panics at compile time if the needle is not found.
pub const fn find_after(bytes: &[u8], start: usize, needle: &[u8]) -> usize {
    let mut i = start;
    while i + needle.len() <= bytes.len() {
        if bytes_match(bytes, i, needle) {
            return i + needle.len();
        }
        i += 1;
    }
    panic!("BVH: needle not found in byte stream");
}

pub const fn bytes_match(bytes: &[u8], start: usize, needle: &[u8]) -> bool {
    let mut j = 0;
    while j < needle.len() {
        if start + j >= bytes.len() || bytes[start + j] != needle[j] {
            return false;
        }
        j += 1;
    }
    true
}

pub const fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            _ => break,
        }
    }
    i
}

/// Like `skip_whitespace` but does not cross newlines — used after header
/// keywords (`Frames:`, `Frame Time:`) so a missing value on the same line
/// fails at parse time rather than silently consuming the next line.
pub const fn skip_inline_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            _ => break,
        }
    }
    i
}

pub const fn skip_to_next_line(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    if i < bytes.len() {
        i += 1; // consume '\n'
    }
    i
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_f32: valid inputs ───────────────────────────────────────────

    #[test]
    fn parses_integer() {
        let (v, end) = parse_f32(b"42", 0);
        assert_eq!(end, 2);
        assert!((v - 42.0).abs() < 1e-6);
    }

    #[test]
    fn parses_decimal() {
        let (v, end) = parse_f32(b"3.14", 0);
        assert_eq!(end, 4);
        assert!((v - 3.14).abs() < 1e-5);
    }

    #[test]
    fn parses_negative_decimal() {
        let (v, end) = parse_f32(b"-2.5", 0);
        assert_eq!(end, 4);
        assert!((v - (-2.5)).abs() < 1e-6);
    }

    #[test]
    fn parses_positive_sign() {
        let (v, _) = parse_f32(b"+1.0", 0);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn parses_zero() {
        let (v, end) = parse_f32(b"0.0", 0);
        assert_eq!(end, 3);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn parses_negative_zero() {
        // -0.0f32 == 0.0f32 per IEEE 754; we do not preserve sign of zero.
        let (v, end) = parse_f32(b"-0.0", 0);
        assert_eq!(end, 4);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn parses_leading_dot() {
        // ".5" is implicitly supported: digit loop starts after the dot.
        let (v, end) = parse_f32(b".5", 0);
        assert_eq!(end, 2);
        assert!((v - 0.5).abs() < 1e-6);
    }

    #[test]
    fn parses_trailing_dot() {
        // "42." is implicitly supported: frac_digits stays 0.
        let (v, end) = parse_f32(b"42.", 0);
        assert_eq!(end, 3);
        assert!((v - 42.0).abs() < 1e-6);
    }

    #[test]
    fn parses_small_positive_exponent() {
        let (v, _) = parse_f32(b"9.27476e+02", 0);
        assert!((v - 927.476).abs() < 0.01);
    }

    #[test]
    fn parses_negative_exponent() {
        // 9.27476e-16 is effectively zero at f32 precision.
        let (v, end) = parse_f32(b"9.27476e-16", 0);
        assert_eq!(end, 11);
        assert!(v.abs() < 1e-10);
    }

    #[test]
    fn parses_negative_value_negative_exponent() {
        let (v, end) = parse_f32(b"-4.96962e-14", 0);
        assert_eq!(end, 12);
        assert!(v.abs() < 1e-10);
    }

    #[test]
    fn parses_uppercase_e() {
        let (v, _) = parse_f32(b"1.5E2", 0);
        assert!((v - 150.0).abs() < 1e-4);
    }

    #[test]
    fn stops_at_whitespace() {
        let (v, end) = parse_f32(b"1.5 rest", 0);
        assert_eq!(end, 3);
        assert!((v - 1.5).abs() < 1e-6);
    }

    #[test]
    fn parses_from_offset() {
        let (v, end) = parse_f32(b"abc 2.5 xyz", 4);
        assert_eq!(end, 7);
        assert!((v - 2.5).abs() < 1e-6);
    }

    #[test]
    fn bvh_sample_value_normalizes_to_near_half() {
        // 9.27476e-16 as a rotation channel: (raw + 720) / 1440 ≈ 0.5.
        let (v, _) = parse_f32(b"9.27476e-16", 0);
        let normalized = (v + 720.0) / 1440.0;
        assert!((normalized - 0.5).abs() < 0.01);
    }

    // ── parse_f32: matches std parsing for representative BVH values ──────

    #[test]
    fn matches_std_parse_for_bvh_like_values() {
        let samples: &[&str] = &[
            "15.3137",
            "84.8855",
            "152.037",
            "-0.188091",
            "9.27476e-16",
            "-3.22702e-10",
            "176.763",
            "-59.1987",
        ];
        for s in samples {
            let (v, end) = parse_f32(s.as_bytes(), 0);
            assert_eq!(end, s.len(), "wrong end index for {s}");
            let expected: f32 = s.parse().unwrap();
            let tolerance = expected.abs().max(1.0) * 1e-5;
            assert!(
                (v - expected).abs() <= tolerance,
                "{s}: got {v}, expected {expected}"
            );
        }
    }

    // ── parse_f32: malformed inputs ───────────────────────────────────────

    #[test]
    #[should_panic(expected = "expected at least one digit")]
    fn parse_f32_rejects_empty() {
        parse_f32(b"", 0);
    }

    #[test]
    #[should_panic(expected = "expected at least one digit")]
    fn parse_f32_rejects_sign_only() {
        parse_f32(b"-", 0);
    }

    #[test]
    #[should_panic(expected = "exponent has no digits")]
    fn parse_f32_rejects_bare_exponent() {
        parse_f32(b"1e", 0);
    }

    #[test]
    #[should_panic(expected = "exponent has no digits")]
    fn parse_f32_rejects_exponent_sign_only() {
        parse_f32(b"1e-", 0);
    }

    #[test]
    #[should_panic(expected = "two decimal points")]
    fn parse_f32_rejects_two_decimal_points() {
        parse_f32(b"1.2.3", 0);
    }

    #[test]
    #[should_panic(expected = "exponent out of supported range")]
    fn parse_f32_rejects_huge_exponent() {
        parse_f32(b"1e999", 0);
    }

    // ── parse_uint ────────────────────────────────────────────────────────

    #[test]
    fn parse_uint_basic() {
        let (v, end) = parse_uint(b"592", 0);
        assert_eq!(v, 592);
        assert_eq!(end, 3);
    }

    #[test]
    fn parse_uint_stops_at_tab() {
        let (v, end) = parse_uint(b"592\t0.00833333", 0);
        assert_eq!(v, 592);
        assert_eq!(end, 3);
    }

    #[test]
    #[should_panic(expected = "expected at least one digit")]
    fn parse_uint_rejects_empty() {
        parse_uint(b"", 0);
    }

    // ── scale_pow10 ───────────────────────────────────────────────────────

    #[test]
    fn scale_up() {
        assert!((scale_pow10(1.0, 3) - 1000.0).abs() < 1e-4);
    }

    #[test]
    fn scale_down() {
        assert!((scale_pow10(1000.0, -3) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn scale_zero_exp() {
        assert!((scale_pow10(42.0, 0) - 42.0).abs() < 1e-6);
    }

    // ── find_after ────────────────────────────────────────────────────────

    #[test]
    fn find_after_returns_index_just_past_needle() {
        let bytes = b"hello MOTION\nframes";
        let i = find_after(bytes, 0, b"MOTION");
        assert_eq!(&bytes[i..i + 1], b"\n");
    }

    #[test]
    fn find_after_from_offset_skips_earlier_occurrence() {
        let bytes = b"MOTION\nmore\nMOTION\ndata";
        let i = find_after(bytes, 7, b"MOTION");
        assert_eq!(i, 18);
    }

    #[test]
    #[should_panic(expected = "needle not found")]
    fn find_after_panics_when_missing() {
        find_after(b"no motion here", 0, b"MOTION");
    }

    // ── bytes_match ───────────────────────────────────────────────────────

    #[test]
    fn bytes_match_exact() {
        assert!(bytes_match(b"hello", 0, b"hello"));
    }

    #[test]
    fn bytes_match_from_offset() {
        assert!(bytes_match(b"abcdef", 2, b"cde"));
    }

    #[test]
    fn bytes_no_match() {
        assert!(!bytes_match(b"hello", 0, b"world"));
    }

    // ── skip_whitespace ───────────────────────────────────────────────────

    #[test]
    fn skip_whitespace_skips_space_tab_newline() {
        let i = skip_whitespace(b"  \t\n  42", 0);
        assert_eq!(i, 6);
    }

    #[test]
    fn skip_whitespace_at_non_whitespace() {
        let i = skip_whitespace(b"abc", 0);
        assert_eq!(i, 0);
    }

    // ── skip_to_next_line ─────────────────────────────────────────────────

    #[test]
    fn skip_to_next_line_consumes_newline() {
        let bytes = b"Frame Time:\t0.008\nnext";
        let i = skip_to_next_line(bytes, 0);
        assert_eq!(&bytes[i..i + 4], b"next");
    }

    // ── parse_bvh_motion_section ──────────────────────────────────────────

    const TINY_BVH: &[u8] = b"\
HIERARCHY\n\
ROOT hip\n\
{\n\
  OFFSET 0 0 0\n\
  CHANNELS 2 Xposition Yposition\n\
}\n\
MOTION\n\
Frames:\t3\n\
Frame Time:\t0.033333\n\
1.0 2.0\n\
-3.5 4.25\n\
0.0 -0.001\n\
";

    // Evaluated at compile time — confirms the function works in const context.
    const TINY_PARSED: [[f32; 2]; 3] = parse_bvh_motion_section::<2, 3>(TINY_BVH);

    #[test]
    fn parses_tiny_bvh_in_const_context() {
        assert!((TINY_PARSED[1][0] - (-3.5)).abs() < 1e-6);
    }

    #[test]
    fn parse_tiny_bvh_motion() {
        let frames = parse_bvh_motion_section::<2, 3>(TINY_BVH);
        assert!((frames[0][0] - 1.0).abs() < 1e-6);
        assert!((frames[0][1] - 2.0).abs() < 1e-6);
        assert!((frames[1][0] - (-3.5)).abs() < 1e-6);
        assert!((frames[1][1] - 4.25).abs() < 1e-6);
        assert!((frames[2][0] - 0.0).abs() < 1e-6);
        assert!((frames[2][1] - (-0.001)).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "Frames count")]
    fn rejects_wrong_frame_count() {
        parse_bvh_motion_section::<2, 4>(TINY_BVH); // TINY_BVH has 3 frames
    }

    const TOO_FEW_VALUES: &[u8] = b"\
MOTION\n\
Frames:\t2\n\
Frame Time:\t0.033333\n\
1.0 2.0\n\
3.0\n\
";

    #[test]
    #[should_panic(expected = "expected at least one digit")]
    fn rejects_too_few_motion_values() {
        parse_bvh_motion_section::<2, 2>(TOO_FEW_VALUES);
    }

    const TOO_MANY_VALUES: &[u8] = b"\
MOTION\n\
Frames:\t1\n\
Frame Time:\t0.033333\n\
1.0 2.0 3.0\n\
";

    #[test]
    #[should_panic(expected = "extra data")]
    fn rejects_extra_motion_values() {
        parse_bvh_motion_section::<2, 1>(TOO_MANY_VALUES);
    }

    #[test]
    #[should_panic(expected = "needle not found")]
    fn rejects_missing_motion_section() {
        parse_bvh_motion_section::<2, 1>(b"Frames: 1\nFrame Time: 0.1\n1.0 2.0\n");
    }

    #[test]
    #[should_panic(expected = "needle not found")]
    fn rejects_missing_frame_time() {
        parse_bvh_motion_section::<2, 1>(
            b"MOTION\nFrames: 1\n1.0 2.0\n",
        );
    }

    #[test]
    #[should_panic(expected = "expected at least one digit")]
    fn rejects_bad_frame_time_value() {
        // Value is on the next line; skip_inline_whitespace does not cross it,
        // so parse_f32 sees '\n' and fails rather than consuming the data row.
        parse_bvh_motion_section::<2, 1>(
            b"MOTION\nFrames: 1\nFrame Time:\n1.0 2.0\n",
        );
    }

    // ── parse_f32: digit-count limit ──────────────────────────────────────

    #[test]
    #[should_panic(expected = "too many significant digits")]
    fn parse_f32_rejects_too_many_digits() {
        parse_f32(b"1234567890123456789", 0); // 19 digits
    }

    // ── parse_bvh_motion_section: exponents and signs ─────────────────────

    const EXP_BVH: &[u8] = b"\
MOTION\n\
Frames:\t1\n\
Frame Time:\t8.33333e-3\n\
+1.0 -2.5 9.27476e-16\n\
";

    #[test]
    fn parses_motion_with_exponents_and_plus_signs() {
        let frames = parse_bvh_motion_section::<3, 1>(EXP_BVH);
        assert!((frames[0][0] - 1.0).abs() < 1e-6);
        assert!((frames[0][1] + 2.5).abs() < 1e-6);
        assert!(frames[0][2].abs() < 1e-10);
    }

    // ── skip_inline_whitespace ────────────────────────────────────────────

    #[test]
    fn skip_inline_whitespace_stops_at_newline() {
        let i = skip_inline_whitespace(b"  \t  \n42", 0);
        assert_eq!(i, 5); // stops before '\n'
    }

    #[test]
    fn skip_inline_whitespace_skips_space_and_tab() {
        let i = skip_inline_whitespace(b"  \t42", 0);
        assert_eq!(i, 3);
    }

    // ── skip_token ────────────────────────────────────────────────────────

    #[test]
    fn skip_token_stops_at_whitespace() {
        let i = skip_token(b"Xrotation next", 0);
        assert_eq!(i, 9);
    }

    #[test]
    fn skip_token_at_end_of_input() {
        let i = skip_token(b"Xrotation", 0);
        assert_eq!(i, 9);
    }

    // ── parse_bvh_channel_is_position ─────────────────────────────────────

    // Standard layout: positions first, then rotations.
    const CHANNEL_BVH: &[u8] = b"\
HIERARCHY\n\
ROOT hip\n\
{\n\
  OFFSET 0 0 0\n\
  CHANNELS 6 Xposition Yposition Zposition Zrotation Yrotation Xrotation\n\
  JOINT chest\n\
  {\n\
    OFFSET 0 5 0\n\
    CHANNELS 3 Zrotation Xrotation Yrotation\n\
    End Site { OFFSET 0 3 0 }\n\
  }\n\
}\n\
MOTION\n\
Frames:\t1\n\
Frame Time:\t0.033\n\
1 2 3 4 5 6 7 8 9\n\
";

    #[test]
    fn channel_scanner_standard_layout() {
        let is_pos = parse_bvh_channel_is_position::<9>(CHANNEL_BVH);
        assert_eq!(
            is_pos,
            [true, true, true, false, false, false, false, false, false]
        );
    }

    // Nonstandard root: positions and rotations interleaved — proves the
    // scanner reads channel names from the file, not `ch < 3`.
    const NONSTANDARD_BVH: &[u8] = b"\
HIERARCHY\n\
ROOT hip\n\
{\n\
  OFFSET 0 0 0\n\
  CHANNELS 6 Zrotation Xposition Yrotation Yposition Xrotation Zposition\n\
  End Site { OFFSET 0 1 0 }\n\
}\n\
MOTION\n\
Frames:\t1\n\
Frame Time:\t0.033\n\
1 2 3 4 5 6\n\
";

    #[test]
    fn channel_scanner_nonstandard_interleaved_order() {
        let is_pos = parse_bvh_channel_is_position::<6>(NONSTANDARD_BVH);
        assert_eq!(is_pos, [false, true, false, true, false, true]);
    }

    #[test]
    #[should_panic(expected = "more channels in file than DOF")]
    fn channel_scanner_rejects_dof_too_small() {
        parse_bvh_channel_is_position::<5>(CHANNEL_BVH); // file has 9 channels
    }

    #[test]
    #[should_panic(expected = "channel count does not match DOF")]
    fn channel_scanner_rejects_dof_too_large() {
        parse_bvh_channel_is_position::<12>(CHANNEL_BVH); // file has 9 channels
    }

    // ── normalize_bvh_motion ──────────────────────────────────────────────

    #[test]
    fn normalize_maps_zero_rotation_to_half() {
        let raw = [[0.0f32; 1]; 1];
        let out = normalize_bvh_motion::<1, 1>(raw, [false], BvhNormalizePolicy::LINKAGE_BLAZE);
        // (0.0 + 720) / 1440 = 0.5, within snap → exactly 0.5
        assert_eq!(out[0][0], 0.5);
    }

    #[test]
    fn normalize_maps_position_and_rotation_correctly() {
        let raw = [[150.0f32, 360.0f32]; 1];
        let is_pos = [true, false];
        let out = normalize_bvh_motion::<2, 1>(raw, is_pos, BvhNormalizePolicy::LINKAGE_BLAZE);
        // position: (150 + 300) / 600 = 0.75
        assert!((out[0][0] - 0.75).abs() < 1e-6);
        // rotation: (360 + 720) / 1440 = 0.75
        assert!((out[0][1] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn normalize_snaps_near_center_to_half() {
        // 0.5% rotation (well within ±0.01 snap band after normalization)
        let raw = [[7.2f32]; 1]; // (7.2 + 720) / 1440 = 0.505, |0.505 - 0.5| = 0.005 ≤ 0.01
        let out = normalize_bvh_motion::<1, 1>(raw, [false], BvhNormalizePolicy::LINKAGE_BLAZE);
        assert_eq!(out[0][0], 0.5);
    }

    #[test]
    fn normalize_does_not_snap_outside_band() {
        // (21.6 + 720) / 1440 = 0.515, |0.515 - 0.5| = 0.015 > 0.01
        let raw = [[21.6f32]; 1];
        let out = normalize_bvh_motion::<1, 1>(raw, [false], BvhNormalizePolicy::LINKAGE_BLAZE);
        assert!((out[0][0] - 0.515).abs() < 1e-6);
    }

    #[test]
    fn normalize_accepts_range_boundaries() {
        // Exact boundaries normalize to 0.0 and 1.0.
        let raw = [[-720.0f32, 720.0f32, -300.0f32, 300.0f32]; 1];
        let is_pos = [false, false, true, true];
        let out = normalize_bvh_motion::<4, 1>(raw, is_pos, BvhNormalizePolicy::LINKAGE_BLAZE);
        assert_eq!(out[0][0], 0.0);
        assert_eq!(out[0][1], 1.0);
        assert_eq!(out[0][2], 0.0);
        assert_eq!(out[0][3], 1.0);
    }

    #[test]
    #[should_panic(expected = "rotation channel value is above")]
    fn normalize_rejects_rotation_above_range() {
        normalize_bvh_motion::<1, 1>([[721.0f32]; 1], [false], BvhNormalizePolicy::LINKAGE_BLAZE);
    }

    #[test]
    #[should_panic(expected = "rotation channel value is below")]
    fn normalize_rejects_rotation_below_range() {
        normalize_bvh_motion::<1, 1>([[-721.0f32]; 1], [false], BvhNormalizePolicy::LINKAGE_BLAZE);
    }

    #[test]
    #[should_panic(expected = "position channel value is above")]
    fn normalize_rejects_position_above_range() {
        normalize_bvh_motion::<1, 1>([[301.0f32]; 1], [true], BvhNormalizePolicy::LINKAGE_BLAZE);
    }

    #[test]
    #[should_panic(expected = "position channel value is below")]
    fn normalize_rejects_position_below_range() {
        normalize_bvh_motion::<1, 1>([[-301.0f32]; 1], [true], BvhNormalizePolicy::LINKAGE_BLAZE);
    }

    // ── norm_to_u16 / u16_to_norm / BvhMotion ────────────────────────────────

    #[test]
    fn u16_endpoints_are_exact() {
        assert_eq!(norm_to_u16(0.0), 0);
        assert_eq!(norm_to_u16(1.0), 65535);
        assert_eq!(u16_to_norm(0), 0.0);
        assert_eq!(u16_to_norm(65535), 1.0);
    }

    #[test]
    fn u16_center_is_exact_by_policy() {
        assert_eq!(norm_to_u16(0.5), PARAM_CENTER_U16);
        assert_eq!(u16_to_norm(PARAM_CENTER_U16), 0.5);
    }

    #[test]
    fn frame_into_expands_one_frame() {
        let motion = BvhMotion::<3, 1>::new([[0, PARAM_CENTER_U16, 65535]]);
        let mut out = [99.0f32; 3];
        motion.frame_into(0, &mut out);
        assert_eq!(out, [0.0, 0.5, 1.0]);
    }
}
