#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod cli;
pub mod config;
pub mod error;
pub use error::{MabelError, Result};
