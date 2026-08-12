#![recursion_limit = "256"]

pub mod analysis;
pub mod audio;
pub mod config;
pub mod i18n;
pub mod keyboard;
pub mod log_protocol;
pub mod log_reader;
pub mod osc;
pub mod runtime;

pub const APP_NAME: &str = "Ecliptica Data Analyzer";
pub const APP_ID: &str = "ecliptica-data-analyzer";
