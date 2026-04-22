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
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use log::{Level, LevelFilter, Log, Metadata, Record};

struct LoggerState {
	file: BufWriter<File>,
	bytes_written: usize,
	created_at: SystemTime,
	log_max_size_bytes: usize,
	log_rotation_interval_secs: u64,
	log_max_files: usize,
}

/// A logger implementation that writes logs to both stderr and a file.
///
/// The logger formats log messages with RFC3339 timestamps and writes them to:
/// - stdout/stderr for console output
/// - A file specified during initialization (if enabled)
///
/// All log messages follow the format:
/// `[TIMESTAMP LEVEL TARGET FILE:LINE] MESSAGE`
///
/// Example: `[2025-12-04T10:30:45Z INFO ldk_server:42] Starting up...`
///
/// The logger does a native size/time-based rotation and retains the last 5 logs by default, if `max_rotated_files` is unset.
pub struct ServerLogger {
	/// The maximum log level to display
	level: LevelFilter,
	/// Groups the file and state in a single Mutex. None if file logging is disabled.
	state: Option<Mutex<LoggerState>>,
	/// Path to the log file for reopening on SIGHUP
	log_file_path: PathBuf,
}

pub struct LogConfig {
	pub log_to_file: bool,
	pub log_max_size_bytes: usize,
	pub log_rotation_interval_secs: u64,
	pub log_max_files: usize,
}

impl ServerLogger {
	/// Initializes the global logger with the specified level and file path.
	///
	/// Opens or creates the log file at the given path. if `log_to_file` is true.
	/// If the file exists, logs are appended.
	/// If the file doesn't exist, it will be created along with any necessary parent directories.
	///
	/// This should be called once at application startup. Subsequent calls will fail.
	///
	/// Returns an Arc to the logger for signal handling purposes.
	pub fn init(
		level: LevelFilter, log_file_path: &Path, log_config: LogConfig,
	) -> Result<Arc<Self>, io::Error> {
		let state = if log_config.log_to_file {
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

			Some(Mutex::new(LoggerState {
				file: BufWriter::new(file),
				bytes_written: initial_size,
				created_at,
				log_max_size_bytes: log_config.log_max_size_bytes,
				log_rotation_interval_secs: log_config.log_rotation_interval_secs,
				log_max_files: log_config.log_max_files,
			}))
		} else {
			None
		};

		let logger =
			Arc::new(ServerLogger { level, log_file_path: log_file_path.to_path_buf(), state });

		log::set_boxed_logger(Box::new(LoggerWrapper(Arc::clone(&logger))))
			.map_err(io::Error::other)?;
		log::set_max_level(level);

		Ok(logger)
	}

	/// Reopens the log file. This flushes the current file writer and opens
	/// the file at `log_file_path` again.
	///
	/// Called on SIGHUP for log rotation.
	pub fn reopen(&self) -> Result<(), io::Error> {
		if let Some(state_mutex) = &self.state {
			if let Ok(mut state) = state_mutex.lock() {
				state.file.flush()?;
				let file = open_log_file(&self.log_file_path)?;

				// Reset size and age tracking for the new file
				let metadata = fs::metadata(&self.log_file_path);
				state.bytes_written = metadata.as_ref().map(|m| m.len() as usize).unwrap_or(0);
				state.created_at = metadata
					.and_then(|m| m.created().or_else(|_| m.modified()))
					.unwrap_or_else(|_| SystemTime::now());

				state.file = BufWriter::new(file);
				return Ok(());
			}
			return Err(io::Error::other("Logger state mutex poisoned"));
		}
		Ok(())
	}

	/// Flushes the current file, renames it with a timestamp, opens a fresh log,
	/// and synchronously deletes older log files.
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

		// Clean up old log files
		if let Err(e) = cleanup_old_logs(&self.log_file_path, state.log_max_files) {
			eprintln!("Failed to clean up old log files: {}", e);
		}

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

			if let Some(state_mutex) = &self.state {
				// Log to file
				let log_bytes = log_line.len() + 1;

				if let Ok(mut state) = state_mutex.lock() {
					let mut needs_rotation = false;

					if state.bytes_written + log_bytes > state.log_max_size_bytes {
						needs_rotation = true;
					} else if let Ok(age) = SystemTime::now().duration_since(state.created_at) {
						if age.as_secs() > state.log_rotation_interval_secs {
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
	}

	fn flush(&self) {
		let _ = io::stdout().flush();
		let _ = io::stderr().flush();

		if let Some(state_mutex) = &self.state {
			if let Ok(mut state) = state_mutex.lock() {
				let _ = state.file.flush();
			}
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

fn cleanup_old_logs(log_file_path: &Path, max_files: usize) -> io::Result<()> {
	let parent = log_file_path.parent().unwrap_or_else(|| Path::new("."));
	let file_name = log_file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
	let mut entries: Vec<_> = fs::read_dir(parent)?
		.filter_map(|entry| entry.ok())
		.filter(|entry| {
			let name = entry.file_name().into_string().unwrap_or_default();
			name.starts_with(file_name) && name != file_name
		})
		.collect();

	// Sort by modification time (oldest first)
	entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::now()));

	if entries.len() > max_files {
		for entry in entries.iter().take(entries.len() - max_files) {
			let _ = fs::remove_file(entry.path());
		}
	}

	Ok(())
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
