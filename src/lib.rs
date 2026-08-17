//! Core library for the Motorbike Engine Simulator.
//!
//! The public API is deliberately small: [`config`] holds validated editable
//! profile data, [`simulation`] is the deterministic fixed-step physics model,
//! and [`audio`] turns simulation snapshots into a procedural output stream.
//! The native GUI remains a private application detail.

#![deny(missing_docs)]

pub mod audio;
pub mod config;
pub mod simulation;
