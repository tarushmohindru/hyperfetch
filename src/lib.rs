use std::time::Duration;

pub mod cli;
pub mod downloader;
pub mod models;
pub mod utils;

pub const DEFAULT_CHUNK_SIZE: u64 = 1024 * 1024; // 1 MB
pub const MAX_CONNECTIONS: usize = 16;
pub const MIN_CHUNK_SIZE: u64 = 64 * 1024; // 64 KB
pub const MAX_CHUNK_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
pub const SPEED_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
pub const READ_TIMEOUT: Duration = Duration::from_secs(60);
