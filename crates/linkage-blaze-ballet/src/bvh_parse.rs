//! Const-fn parser for the BVH motion section.
//!
//! Parses the numeric subset used in BVH motion files:
//!
//!   `['-'|'+'] digits ['.' digits] [('e'|'E') ['-'|'+'] digits]`
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
//! but it is deterministic, allocation-free, and accurate to within a few ULPs
//! for BVH motion data, which carries at most 9 significant digits.

// todo000 article: Parsing a 764 KB BVH file in a Rust const fn takes ~8 s.
// Is that acceptable for an embedded project?  Weigh against the alternative:
// a `just generate-ballet` code-generation step checked in as
// `ballet_frames_precomputed.rs`.  Neither answer is clearly right.

const MAX_MANTISSA_DIGITS: usize = 18;

// ── public API ───────────────────────────────────────────────────────────────

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
    i = skip_whitespace(bytes, i);
    let (frame_count, next) = parse_uint(bytes, i);
    assert!(frame_count == FRAMES, "BVH Frames count does not match FRAMES");
    i = skip_to_next_line(bytes, next);

    // "Frame Time:\t<value>\n" — parse to confirm structure, discard value.
    i = find_after(bytes, i, b"Frame Time:");
    let (_, next) = parse_f32(bytes, skip_whitespace(bytes, i));
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

    let mut value = mantissa as f32;
    value = scale_pow10(value, exp10);
    if negative {
        value = -value;
    }

    (value, i)
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
}
