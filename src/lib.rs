//! TOML Virtual Table.
//!
//! Reads TOML files and exposes them as a single virtual table where the content of each file can be queried as JSON.

mod reader;
mod types;

#[cfg(feature = "rlib")]
mod rust_lib;

#[cfg(feature = "rlib")]
pub use rust_lib::load_module;

#[cfg(feature = "loadable")]
mod standalone;

#[cfg(feature = "loadable")]
pub use standalone::sqlite3_tomlvtab_init;
