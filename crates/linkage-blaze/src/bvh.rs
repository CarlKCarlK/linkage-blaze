//! Support for the Biovision Hierarchy (BVH) motion-capture file format.
//!
//! [`Motion`] is always available: it parses fixed-size, normalized motion at
//! compile time without allocation and works in `no_std` firmware. The
//! optional `bvh` feature adds host-side parsing and conversion APIs such as
//! [`parse`], [`Clip`], and [`build_linkage_buf`].
//!
//! Use `bvh::motion!` for embedded motion data. On a host,
//! parse a complete BVH file with [`parse`], discover its linkage parameters,
//! and convert it with [`build_linkage_buf`].

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
    Channel, Clip, Error, Joint, MotionSample as Sample, Parameter, ParameterLayout,
    build_bvh_linkage_buf as build_linkage_buf, bvh_sample_params as sample_params,
    bvh_to_lb_rs as to_lb_rs, discover_bvh_parameters as discover_parameters, parse_bvh as parse,
};
