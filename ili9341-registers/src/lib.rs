//! # Ilitek ILI9341 TFT LCD Controller Commands
//!
//! This crate provides the ILI9341 command set and typed parameter layouts.
//!
//! The ILI9341 has no addressable register file: the host sends a command byte with the
//! `D/CX` line low, followed by its parameter bytes with `D/CX` high. [`Command`] lists the
//! command codes; the parameter types in this crate encode the bit layouts of the
//! parameters that carry flags.
//!
//! Command codes and bit layouts follow the ILI9341 datasheet (V1.11).

#![deny(missing_docs)]
#![no_std]
#![forbid(unsafe_code)]
// bitfield-struct generates `with_*_checked` setters that return `Result<_, ()>`.
#![allow(clippy::result_unit_err)]
// Enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod commands;
mod parameters;

pub use commands::*;
pub use parameters::*;

/// The number of columns of the frame memory (the short side of the panel).
pub const WIDTH: u16 = 240;

/// The number of rows of the frame memory (the long side of the panel).
pub const HEIGHT: u16 = 320;
