use std::{collections::HashMap, time::Duration};

use crate::{DEFAULT_CHUNK_SIZE, MAX_CONNECTIONS};

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: f64,
    pub eta_seconds: u64,
    pub active_connections: usize,
    pub chunks_completed: usize,
    pub chunks_total: usize,
}

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub max_connections: usize,
    pub chunk_size: u64,
    pub user_agent: String,
    pub headers: HashMap<String, String>,
    pub retry_attempts: usize,
    pub retry_delay: Duration,
    pub adaptive_chunking: bool,
    pub resume_support: bool,
    pub connection_reuse: bool,
    pub compression: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "*/*".to_string());
        headers.insert(
            "Accept-Encoding".to_string(),
            "gzip, deflate, br".to_string(),
        );
        headers.insert("Connection".to_string(), "keep-alive".to_string());

        Self {
            max_connections: MAX_CONNECTIONS,
            chunk_size: DEFAULT_CHUNK_SIZE,
            user_agent: "RustDownloader/1.0".to_string(),
            headers,
            retry_attempts: 3,
            retry_delay: Duration::from_millis(1000),
            adaptive_chunking: true,
            resume_support: true,
            connection_reuse: true,
            compression: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub completed: bool,
    pub retries: usize,
}

#[derive(Debug)]
pub struct ChunkCompleted(pub usize);
