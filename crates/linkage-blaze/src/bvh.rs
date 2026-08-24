//! Compile-time and host-side support for the Biovision Hierarchy (BVH)
//! motion-capture file format.
//!
//! [`Motion`] is always available and parses normalized, fixed-size motion at
//! compile time without allocation. The optional `bvh` feature adds host-side
//! parsing and conversion APIs such as [`parse`] and [`Clip`].

#[path = "bvh_motion.rs"]
mod motion;

pub use motion::Motion;

/// Embed and normalize a BVH motion-capture file at compile time.
pub use crate::__bvh_motion as motion;

#[cfg(feature = "bvh")]
#[path = "bvh_host.rs"]
mod host;

#[cfg(feature = "bvh")]
pub use host::{
    BvhChannel as Channel, BvhClip as Clip, BvhJoint as Joint, BvhParameter as Parameter,
    BvhParameterLayout as ParameterLayout, Error, MotionSample as Sample,
    build_bvh_linkage_buf as build_linkage_buf, bvh_sample_params as sample_params,
    bvh_to_lb_rs as to_lb_rs, discover_bvh_parameters as discover_parameters, parse_bvh as parse,
};
