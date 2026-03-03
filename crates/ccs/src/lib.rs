pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod cli;
pub mod color;
pub mod config;
pub mod doctor;
pub mod error;
pub mod launcher;
pub mod presets;
pub mod profiles;
pub mod state;
pub mod tester;
pub mod wizard;
