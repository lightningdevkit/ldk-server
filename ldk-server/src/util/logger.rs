// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Maximum size of the log file before it gets rotated (50 MB)
const MAX_LOG_SIZE_BYTES: usize = 50 * 1024 * 1024;
/// Maximum age of the log file before it gets rotated (24 hours)
const ROTATION_INTERVAL_SECS: u64 = 24 * 60 * 60;

struct LoggerState {
	file: BufWriter<File>,
	bytes_written: usize,
	created_at: SystemTime,
}

/// A logger implementation that writes logs to both stderr and a file.
///
/// The logger formats log messages with RFC3339 timestamps and writes them to:
/// - stdout/stderr for console output
/// - A file specified during initialization
///
/// All log messages follow the format:
/// `[TIMESTAMP LEVEL TARGET FILE:LINE] MESSAGE`
///
/// Example: `[2025-12-04T10:30:45Z INFO ldk_server:42] Starting up...`
///
/// The logger does a native size/time-based rotation and zero-dependency background gzip compression.
pub struct ServerLogger {
	/// The maximum log level to display
	level: LevelFilter,
	/// Groups the file and state in a single Mutex
	state: Mutex<LoggerState>,
	/// Path to the log file for reopening on SIGHUP
	log_file_path: PathBuf,
}

impl ServerLogger {
	/// Initializes the global logger with the specified level and file path.
	///
	/// Opens or creates the log file at the given path. If the file exists, logs are appended.
	/// If the file doesn't exist, it will be created along with any necessary parent directories.
	///
	/// This should be called once at application startup. Subsequent calls will fail.
	///
	/// Returns an Arc to the logger for signal handling purposes.
	pub fn init(level: LevelFilter, log_file_path: &Path) -> Result<Arc<Self>, io::Error> {
		// Create parent directories if they don't exist
		if let Some(parent) = log_file_path.parent() {
			fs::create_dir_all(parent)?;
		}

		let file = open_log_file(log_file_path)?;

		// Check existing file metadata to persist size and age across node restarts
		let metadata = fs::metadata(log_file_path);
		let initial_size = metadata.as_ref().map(|m| m.len() as usize).unwrap_or(0);
		let created_at = metadata
			.and_then(|m| m.created().or_else(|_| m.modified()))
			.unwrap_or_else(|_| SystemTime::now());

		let logger = Arc::new(ServerLogger {
			level,
			log_file_path: log_file_path.to_path_buf(),
			state: Mutex::new(LoggerState {
				file: BufWriter::new(file),
				bytes_written: initial_size,
				created_at,
			}),
		});

		log::set_boxed_logger(Box::new(LoggerWrapper(Arc::clone(&logger))))
			.map_err(io::Error::other)?;
		log::set_max_level(level);

		Ok(logger)
	}

	/// Flushes the current file, renames it with a timestamp, opens a fresh log,
	/// and spawns a background thread to compress the old file.
	fn rotate(&self, state: &mut LoggerState) -> Result<(), io::Error> {
		state.file.flush()?;

		let now = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
		let mut new_path = self.log_file_path.to_path_buf().into_os_string();
		new_path.push(".");
		new_path.push(now);
		let rotated_path = PathBuf::from(new_path);

		fs::rename(&self.log_file_path, &rotated_path)?;

		let new_file = open_log_file(&self.log_file_path)?;
		state.file = BufWriter::new(new_file);

		// Reset our rotation triggers for the new file
		state.bytes_written = 0;
		state.created_at = SystemTime::now();

		// Spawn independent OS thread to compress the old file using native gzip
		thread::spawn(move || match Command::new("gzip").arg("-f").arg(&rotated_path).status() {
			Ok(status) if status.success() => {},
			Ok(status) => {
				eprintln!("Failed to compress log {:?}: exited with {}", rotated_path, status)
			},
			Err(e) => eprintln!("Failed to execute gzip on {:?}: {}", rotated_path, e),
		});

		Ok(())
	}
}

impl Log for ServerLogger {
	fn enabled(&self, metadata: &Metadata) -> bool {
		metadata.level() <= self.level
	}

	fn log(&self, record: &Record) {
		if self.enabled(record.metadata()) {
			let level_str = format_level(record.level());
			let line = record.line().unwrap_or(0);

			let log_line = format!(
				"[{} {} {}:{}] {}",
				format_timestamp(),
				level_str,
				record.target(),
				line,
				record.args()
			);

			// Log to console
			match record.level() {
				Level::Error => {
					let _ = writeln!(io::stderr(), "{}", log_line);
				},
				_ => {
					let _ = writeln!(io::stdout(), "{}", log_line);
				},
			};

			// Log to file
			let log_bytes = log_line.len() + 1;

			if let Ok(mut state) = self.state.lock() {
				let mut needs_rotation = false;

				if state.bytes_written + log_bytes > MAX_LOG_SIZE_BYTES {
					needs_rotation = true;
				} else if let Ok(age) = SystemTime::now().duration_since(state.created_at) {
					if age.as_secs() > ROTATION_INTERVAL_SECS {
						needs_rotation = true;
					}
				}

				if needs_rotation {
					if let Err(e) = self.rotate(&mut state) {
						eprintln!("Failed to rotate log file: {}", e);
					}
				}

				let _ = writeln!(state.file, "{}", log_line);
				state.bytes_written += log_bytes;
			}
		}
	}

	fn flush(&self) {
		let _ = io::stdout().flush();
		let _ = io::stderr().flush();
		if let Ok(mut state) = self.state.lock() {
			let _ = state.file.flush();
		}
	}
}

fn format_timestamp() -> String {
	let now = chrono::Utc::now();
	now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn format_level(level: Level) -> &'static str {
	match level {
		Level::Error => "ERROR",
		Level::Warn => "WARN ",
		Level::Info => "INFO ",
		Level::Debug => "DEBUG",
		Level::Trace => "TRACE",
	}
}

fn open_log_file(log_file_path: &Path) -> Result<File, io::Error> {
	OpenOptions::new().create(true).append(true).open(log_file_path)
}

/// Wrapper to allow Arc<ServerLogger> to implement Log trait
struct LoggerWrapper(Arc<ServerLogger>);

impl Log for LoggerWrapper {
	fn enabled(&self, metadata: &Metadata) -> bool {
		self.0.enabled(metadata)
	}

	fn log(&self, record: &Record) {
		self.0.log(record)
	}

	fn flush(&self) {
		self.0.flush()
	}
}
