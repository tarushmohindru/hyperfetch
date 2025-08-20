use clap::Parser;
use console::style;
use hyperfetch::{
    cli::Cli,
    downloader::Downloader,
    models::{DownloadConfig, DownloadProgress},
    utils::{extract_filename_from_url, format_bytes, format_duration, format_speed, parse_size},
};
use indicatif::{ProgressBar, ProgressStyle};
use std::{collections::HashMap, fs, io::Write, path::Path, time::Duration};
use tokio::time::Instant;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let url = cli.url;
    let output_path = cli
        .output
        .clone()
        .unwrap_or_else(|| extract_filename_from_url(&url));

    let max_connections = cli.connections;

    let chunk_size = parse_size(&cli.chunk_size)?;

    let mut headers = HashMap::new();
    if let Some(header_values) = cli.header {
        for header in header_values {
            if let Some((name, value)) = header.split_once(":") {
                headers.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }

    let retry_attempts = cli.retries;

    let _timeout_secs = cli.timeout;

    let mut config = DownloadConfig {
        max_connections,
        chunk_size,
        headers,
        retry_attempts,
        retry_delay: Duration::from_secs(1),
        resume_support: !cli.no_resume,
        adaptive_chunking: !cli.no_adaptive,
        ..Default::default()
    };

    if let Some(user_agent) = cli.user_agent {
        config.user_agent = user_agent.clone();
    }

    let quite = cli.quiet;
    let verbose = cli.verbose;

    let downloader = Downloader::new(config);

    if verbose {
        println!("{}", style("Dart Downloader").bold().cyan());
        println!("URL: {}", url);
        println!("Output: {}", output_path);
        println!("Max connections: {}", max_connections);
        println!("Chunk size: {}", format_bytes(chunk_size));
        println!("Resume support: {}", !cli.no_resume);
        println!("Adaptive chunking: {}", !cli.no_adaptive);
        println!();
    }

    let progress_bar = if !quite {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::default_bar().
            template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) ETA: {eta} | {msg}")
            ?.progress_chars("##-"),
        );
        Some(pb)
    } else {
        None
    };

    let start_time = Instant::now();

    let progress_callback = if let Some(pb) = progress_bar.clone() {
        let pb_clone = pb.clone();
        let _start_time = start_time;

        Some(Box::new(move |progress: DownloadProgress| {
            pb_clone.set_length(progress.total_bytes);
            pb_clone.set_position(progress.downloaded_bytes);

            let percentage = if progress.total_bytes > 0 {
                (progress.downloaded_bytes as f64 / progress.total_bytes as f64) * 100.0
            } else {
                0.0
            };

            let message = format!(
                "{:.1}% | {} connections | {}/{} chunks",
                percentage,
                progress.active_connections,
                progress.chunks_completed,
                progress.chunks_total
            );
            pb_clone.set_message(message);
        }) as Box<dyn Fn(DownloadProgress) + Send + Sync>)
    } else if verbose {
        Some(Box::new(move |progress: DownloadProgress| {
            let percentage = if progress.total_bytes > 0 {
                (progress.downloaded_bytes as f64 / progress.total_bytes as f64) * 100.0
            } else {
                0.0
            };

            print!(
                "\r{:.1}% {} / {} ({}) ETA: {} | {} connections",
                percentage,
                format_bytes(progress.downloaded_bytes),
                format_bytes(progress.total_bytes),
                format_speed(progress.speed_bps),
                format_duration(progress.eta_seconds),
                progress.active_connections
            );
            let _ = std::io::stdout().flush();
        }) as Box<dyn Fn(DownloadProgress) + Send + Sync>)
    } else {
        None
    };

    let result = downloader
        .download(&url, &output_path, progress_callback)
        .await;

    if let Some(pb) = progress_bar.as_ref() {
        pb.finish();
    }

    match result {
        Ok(_) => {
            let elapsed = start_time.elapsed();
            let final_progress = downloader.get_progress();
            let mut final_path = output_path.clone();

            let path = Path::new(&output_path);
            if path.extension().is_none() {
                println!(
                    "DEBUG: Filename '{}' has no extension. Attempting to infer type from content...",
                    output_path
                );
            }
            match infer::get_from_path(&output_path) {
                Ok(Some(kind)) => {
                    let extension = kind.extension();
                    let new_path = format!("{}.{}", output_path, extension);
                    if fs::rename(output_path, &new_path).is_ok() {
                        final_path = new_path;
                        println!("DEBUG: Renamed file to '{}'", final_path);
                    } else {
                        eprintln!("DEBUG: Error: Failed to rename the file.");
                    }
                }
                Ok(None) => {
                    println!(
                        "DEBUG: Could not infer the file type. The format is unknown or not supported by 'infer'."
                    );
                }
                Err(e) => {
                    eprintln!(
                        "DEBUG: An error occurred while trying to read the file for inference: {}",
                        e
                    );
                }
            }

            if !quite {
                println!();
                println!(
                    "{}",
                    style("Download completed successfully!").green().bold()
                );
                println!("File: {}", final_path);
                println!("Size: {}", format_bytes(final_progress.total_bytes));
                println!("Time: {}", format_duration(elapsed.as_secs()));
                println!(
                    "Average speed: {}",
                    format_speed(final_progress.total_bytes as f64 / elapsed.as_secs_f64())
                );
            }
        }
        Err(e) => {
            if !quite {
                println!();
                eprintln!("{} {}", style("Error:").red().bold(), e);
            }

            std::process::exit(1);
        }
    }

    Ok(())
}
