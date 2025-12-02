#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[allow(unused_imports)]
#[macro_use]
extern crate log;

// HACK: used by delegate macros to access xash3d_server types
#[doc(hidden)]
extern crate self as xash3d_server;

#[macro_use]
pub mod macros;

pub mod change_level;
pub mod consts;
pub mod cvar;
pub mod engine;
pub mod entities;
pub mod entity;
pub mod export;
pub mod game_rules;
pub mod global_state;
pub mod globals;
pub mod instance;
mod logger;
pub mod prelude;
pub mod private;
pub mod save;
pub mod sound;
pub mod str;
pub mod studio;
pub mod time;
pub mod user_message;
pub mod utils;

pub use xash3d_shared::{cell, color, csz, ffi, math, parser, render};
