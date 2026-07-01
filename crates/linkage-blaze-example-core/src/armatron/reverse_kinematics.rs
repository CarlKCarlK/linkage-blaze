//! Parked reverse-kinematics notes for the armatron example.
//!
//! The live [`super::armatron`] loop is currently manual-only: the player moves
//! the arm sliders directly and watches the distance-to-target report. The RK
//! play/step controls are still drawn as visual placeholders, but touch dispatch
//! intentionally ignores them.
//!
//! Keep this module around as the re-entry point for restoring RK later. The
//! old implementation was useful enough to preserve conceptually, but it should
//! not leak any solver state back into the main game loop until the ownership
//! boundary is clearer.
//!
//! Things to restore when RK comes back:
//!
//! - A small run state, previously `ReverseKinematicsRun`, that records the
//!   current visible target params, search step size, and whether a solve is
//!   actively playing.
//! - A search phase enum, previously `ReverseKinematicsPhase`, that alternates
//!   between arm bend/spin candidate search and visible param interpolation.
//! - Button dispatch for the RK play and single-step controls. Those buttons
//!   are still rendered by `main.rs`; they just do not produce an
//!   `ActiveControl` today.
//! - Per-frame ticking after touch input. The old loop made drawing dependent
//!   on whether the RK tick changed params, so restore that as an explicit part
//!   of frame scheduling instead of hiding it inside touch handling.
//! - The paired-candidate search over bend and whole-arm spin. Keep that logic
//!   near the solver state, not mixed into slider/touch code.
//!
//! The distance-to-target code deliberately remains in `main.rs`. It is part of
//! the manual game: users can move the robot arm themselves and try to minimize
//! the displayed distance.
//!
//! Old constants and helper names to look for in history:
//!
//! ```text
//! RK_INITIAL_STEP
//! RK_MIN_STEP
//! RK_STEP_DECAY
//! RK_VISIBLE_PARAM_POINTS_PER_SECOND
//! RK_MAX_TICK_SECONDS
//! RK_SINGLE_STEP_VISIBLE_PARAM_STEP
//! RK_SEARCH_CANDIDATES_PER_TICK
//! RK_PAIRED_CANDIDATES
//! BEND_ELBOW_PARAM
//! ReverseKinematicsRun
//! ReverseKinematicsPhase
//! toggle_reverse_kinematics
//! clear_reverse_kinematics
//! ensure_reverse_kinematics_run
//! tick_reverse_kinematics_at
//! tick_reverse_kinematics
//! step_reverse_kinematics
//! move_params_toward
//! reverse_kinematics_visible_param_step
//! apply_paired_candidate
//! ```
