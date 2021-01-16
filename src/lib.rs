/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `rocket-static-files` is a simple (but still very rough) way to add caching
//! headers to your static files served by Rocket. Obviously you should use
//! a real HTTP server like nginx or apache, but since you're here, you clearly
//! don't want to.
//!
//! The headers are added in three steps:
//!
//! 1. First, `rocket-static-files` scans your static files directory,
//!    generating a hash for each file, as part of your build script.
//! 2. You add a fairing to your Rocket, replacing rocket_contrib's `serve`
//!    fairing if you're using it.
//! 3. You update your HTML to link to `StaticFiles::to` instead of directly to
//!    the path.
//!
//! If all goes according to plan, your links will look something like:
//! `/static/some_file.png?v=H8y4bzqH6Mg`. When you change your static file and
//! recompile, you'll get a new value for `v`.

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![feature(decl_macro)]

#[macro_use]
extern crate rocket;
#[doc(hidden)]
pub extern crate phf;

mod fairing;
#[cfg(feature = "gen")]
mod gen;

pub use crate::fairing::*;
#[cfg(feature = "gen")]
pub use crate::gen::*;
