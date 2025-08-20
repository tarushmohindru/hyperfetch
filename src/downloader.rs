use crate::{
    CONNECTION_TIMEOUT, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE, READ_TIMEOUT, SPEED_SAMPLE_INTERVAL,
    models::{ChunkCompleted, ChunkInfo, DownloadConfig, DownloadProgress},
};
use reqwest::{
    Client,
    header::{HeaderMap, HeaderName, HeaderValue, RANGE},
};
use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{Semaphore, mpsc},
    time::{interval, sleep},
};

pub struct Downloader {
    client: Client,
    config: DownloadConfig,
    progress: Arc<Mutex<DownloadProgress>>,
    downloaded_bytes: Arc<AtomicU64>,
    should_stop: Arc<AtomicBool>,
    speed_tracker: Arc<Mutex<Vec<(Instant, u64)>>>,
    active_connections: Arc<AtomicUsize>,
}

impl Downloader {
    pub fn new(config: DownloadConfig) -> Self {
        let mut headers = HeaderMap::new();
        for (key, value) in &config.headers {
            if let (Ok(name), Ok(val)) = (key.parse::<HeaderName>(), HeaderValue::from_str(&value))
            {
                headers.insert(name, val);
            }
        }

        let client = Client::builder()
            .timeout(CONNECTION_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .default_headers(headers)
            .user_agent(&config.user_agent)
            .gzip(config.compression)
            .brotli(config.compression)
            .deflate(config.compression)
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(config.max_connections)
            .build()
            .expect("Failed to create HTTP client");

        let progress = DownloadProgress {
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0.0,
            eta_seconds: 0,
            active_connections: 0,
            chunks_completed: 0,
            chunks_total: 0,
        };

        Self {
            client,
            config,
            progress: Arc::new(Mutex::new(progress)),
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            should_stop: Arc::new(AtomicBool::new(false)),
            speed_tracker: Arc::new(Mutex::new(Vec::new())),
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn download<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_callback: Option<Box<dyn Fn(DownloadProgress) + Send + Sync>>,
    ) -> Result<(), anyhow::Error> {
        let output_path = output_path.as_ref();
        if output_path == Path::new("downloaded_file") {}

        self.downloaded_bytes.store(0, Ordering::SeqCst);
        self.should_stop.store(false, Ordering::SeqCst);
        self.speed_tracker.lock().unwrap().clear();

        let (total_size, supports_ranges) = self.get_file_info(url).await?;

        let existing_size = if output_path.exists() && self.config.resume_support {
            std::fs::metadata(output_path)?.len()
        } else {
            0
        };

        // Only consider file complete if we know the total size and existing size matches
        if total_size > 0 && existing_size >= total_size {
            println!("File already downloaded.");
            return Ok(());
        }

        self.downloaded_bytes.store(existing_size, Ordering::SeqCst);

        {
            let mut progress = self.progress.lock().unwrap();
            progress.total_bytes = total_size;
            progress.downloaded_bytes = existing_size;
        }

        let progress_tracker = self.start_progress_tracking(progress_callback);

        if supports_ranges && total_size > self.config.chunk_size {
            self.download_multi_thread(url, output_path, total_size, existing_size)
                .await?
        } else {
            self.download_single_thread(url, output_path, existing_size)
                .await?;
        }

        progress_tracker.abort();

        Ok(())
    }

    async fn get_file_info(&self, url: &str) -> Result<(u64, bool), anyhow::Error> {
        let response = self.client.head(url).send().await?;

        let total_size = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let supports_ranges = response
            .headers()
            .get("accept-ranges")
            .map(|v| v.as_bytes() == b"bytes")
            .unwrap_or(false);

        Ok((total_size, supports_ranges))
    }

    async fn download_single_thread<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        start_from: u64,
    ) -> Result<(), anyhow::Error> {
        // Reflect a single active connection during single-threaded download
        self.active_connections.fetch_add(1, Ordering::SeqCst);
        let mut file = if start_from > 0 {
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(output_path)?
        } else {
            File::create(output_path)?
        };

        let mut request = self.client.get(url);

        if start_from > 0 {
            request = request.header(RANGE, format!("bytes={}-", start_from));
        }

        let mut response = request.send().await?;

        while let Some(chunk) = response.chunk().await? {
            if self.should_stop.load(Ordering::SeqCst) {
                break;
            }

            file.write_all(&chunk)?;
            self.downloaded_bytes
                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
        }

        // Done with the single connection
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }

    async fn download_multi_thread<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        total_size: u64,
        start_from: u64,
    ) -> Result<(), anyhow::Error> {
        let output_path = output_path.as_ref();

        let file = if start_from > 0 {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(output_path)?
        } else {
            File::create(output_path)?
        };

        file.set_len(total_size)?;
        drop(file);

        let remaining_bytes = total_size - start_from;
        let chunk_size = if self.config.adaptive_chunking {
            self.calculate_adaptive_chunk_size(remaining_bytes)
        } else {
            self.config.chunk_size
        };

        let chunks = self.create_chunks(start_from, total_size, chunk_size);
        let total_chunks = chunks.len();

        {
            let mut progress = self.progress.lock().unwrap();
            progress.chunks_total = total_chunks;
        }

        let semaphore = Arc::new(Semaphore::new(self.config.max_connections));
        let (tx, mut rx) = mpsc::channel(100);

        let chunks = Arc::new(Mutex::new(chunks));
        let output_path = Arc::new(output_path.to_owned());

        let mut handles = Vec::new();
        for _ in 0..self.config.max_connections {
            let semaphore = semaphore.clone();
            let client = self.client.clone();
            let chunks = chunks.clone();
            let output_path = output_path.clone();
            let tx = tx.clone();
            let url = url.to_string();
            let downloaded_bytes = self.downloaded_bytes.clone();
            let should_stop = self.should_stop.clone();
            let retry_attempts = self.config.retry_attempts.clone();
            let retry_delay = self.config.retry_delay.clone();
            let active_connections = self.active_connections.clone();

            let handle = tokio::spawn(async move {
                while !should_stop.load(Ordering::SeqCst) {
                    let _permit = semaphore.acquire().await.unwrap();

                    let chunk = {
                        let mut chunks_guard = chunks.lock().unwrap();
                        chunks_guard
                            .iter_mut()
                            .find(|c| !c.completed && c.retries < retry_attempts)
                            .map(|c| {
                                c.retries += 1;
                                c.clone()
                            })
                    };

                    if let Some(mut chunk) = chunk {
                        // Increment active connections for this in-flight chunk
                        active_connections.fetch_add(1, Ordering::SeqCst);
                        let res = Self::download_chunk(
                            &client,
                            &url,
                            &output_path,
                            &mut chunk,
                            &downloaded_bytes,
                        )
                        .await;
                        match res {
                            Ok(_) => {
                                {
                                    let mut chunks_guard = chunks.lock().unwrap();
                                    if let Some(c) =
                                        chunks_guard.iter_mut().find(|c| c.id == chunk.id)
                                    {
                                        c.completed = true;
                                        c.downloaded = chunk.end - chunk.start + 1;
                                    }
                                }

                                let _ = tx.send(ChunkCompleted(chunk.id)).await;
                            }
                            Err(e) => {
                                eprintln!("Chunk {} failed: {}", chunk.id, e);
                                sleep(retry_delay).await;
                            }
                        }
                        // Decrement active connections when this chunk attempt finishes
                        active_connections.fetch_sub(1, Ordering::SeqCst);
                    } else {
                        break;
                    }
                }
            });
            handles.push(handle);
        }

        drop(tx);

        let mut completed_chunks = 0;
        while let Some(ChunkCompleted(_)) = rx.recv().await {
            completed_chunks += 1;
            {
                let mut progress = self.progress.lock().unwrap();
                progress.chunks_completed = completed_chunks;
            }

            if completed_chunks >= total_chunks {
                break;
            }
        }

        for handle in handles {
            handle.abort();
        }

        Ok(())
    }

    async fn download_chunk(
        client: &Client,
        url: &str,
        output_path: &Path,
        chunk: &mut ChunkInfo,
        downloaded_bytes: &Arc<AtomicU64>,
    ) -> Result<(), anyhow::Error> {
        let range_header = format!("bytes={}-{}", chunk.start, chunk.end);
        let mut response = client.get(url).header(RANGE, range_header).send().await?;

        let mut file = OpenOptions::new().write(true).open(output_path)?;
        file.seek(SeekFrom::Start(chunk.start))?;

        let mut bytes_written: u64 = 0;

        while let Some(chunk_bytes) = response.chunk().await? {
            let remaining = (chunk.end - chunk.start + 1) - bytes_written;
            if remaining == 0 {
                break;
            }

            let write_size = std::cmp::min(chunk_bytes.len(), remaining as usize);
            file.write_all(&chunk_bytes[..write_size])?;

            bytes_written += write_size as u64;
            downloaded_bytes.fetch_add(write_size as u64, Ordering::SeqCst);

            if bytes_written >= chunk.end - chunk.start + 1 {
                break;
            }
        }

        chunk.downloaded = bytes_written;
        Ok(())
    }

    fn start_progress_tracking(
        &self,
        callback: Option<Box<dyn Fn(DownloadProgress) + Send + Sync>>,
    ) -> tokio::task::JoinHandle<()> {
        let progress = self.progress.clone();
        let downloaded_bytes = self.downloaded_bytes.clone();
        let speed_tracker = self.speed_tracker.clone();
        let should_stop = self.should_stop.clone();
        let active_connections = self.active_connections.clone();

        tokio::spawn(async move {
            let mut interval = interval(SPEED_SAMPLE_INTERVAL);
            let mut last_bytes = 0u64;
            let mut last_time = Instant::now();

            while !should_stop.load(Ordering::SeqCst) {
                interval.tick().await;

                let current_bytes = downloaded_bytes.load(Ordering::SeqCst);
                let current_time = Instant::now();

                // Calculate speed
                let bytes_diff = current_bytes.saturating_sub(last_bytes);
                let time_diff = current_time.duration_since(last_time).as_secs_f64();
                let current_speed = if time_diff > 0.0 {
                    bytes_diff as f64 / time_diff
                } else {
                    0.0
                };

                // Update speed tracker
                {
                    let mut tracker = speed_tracker.lock().unwrap();
                    tracker.push((current_time, current_bytes));
                    // Keep only last 10 samples
                    if tracker.len() > 10 {
                        tracker.remove(0);
                    }
                }

                // Calculate average speed and ETA
                let (avg_speed, eta) = {
                    let tracker = speed_tracker.lock().unwrap();
                    if tracker.len() >= 2 {
                        let first = tracker.first().unwrap();
                        let last = tracker.last().unwrap();
                        let time_span = last.0.duration_since(first.0).as_secs_f64();
                        let bytes_span = last.1.saturating_sub(first.1);

                        let avg_speed = if time_span > 0.0 {
                            bytes_span as f64 / time_span
                        } else {
                            0.0
                        };

                        let progress_guard = progress.lock().unwrap();
                        let remaining = progress_guard.total_bytes.saturating_sub(current_bytes);
                        let eta = if avg_speed > 0.0 {
                            remaining as f64 / avg_speed
                        } else {
                            0.0
                        };

                        (avg_speed, eta as u64)
                    } else {
                        (current_speed, 0)
                    }
                };

                // Update progress
                {
                    let mut progress_guard = progress.lock().unwrap();
                    progress_guard.downloaded_bytes = current_bytes;
                    progress_guard.speed_bps = avg_speed;
                    progress_guard.eta_seconds = eta;
                    progress_guard.active_connections = active_connections.load(Ordering::SeqCst);
                }

                // Call callback
                if let Some(ref callback) = callback {
                    let progress_copy = progress.lock().unwrap().clone();
                    callback(progress_copy);
                }

                last_bytes = current_bytes;
                last_time = current_time;
            }
        })
    }

    fn create_chunks(&self, start: u64, total_size: u64, chunk_size: u64) -> Vec<ChunkInfo> {
        let mut chunks = Vec::new();
        let mut current_start = start;
        let mut chunk_id = 0;

        while current_start < total_size {
            let chunk_end = std::cmp::min(current_start + chunk_size - 1, total_size - 1);

            chunks.push(ChunkInfo {
                id: chunk_id,
                start: current_start,
                end: chunk_end,
                downloaded: 0,
                completed: false,
                retries: 0,
            });

            current_start = chunk_end + 1;
            chunk_id += 1;
        }

        chunks
    }

    fn calculate_adaptive_chunk_size(&self, remaining_bytes: u64) -> u64 {
        let base_chunk_size = remaining_bytes / (self.config.max_connections as u64 * 4);
        std::cmp::max(
            MIN_CHUNK_SIZE,
            std::cmp::min(MAX_CHUNK_SIZE, base_chunk_size),
        )
    }

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }

    pub fn get_progress(&self) -> DownloadProgress {
        self.progress.lock().unwrap().clone()
    }
}
