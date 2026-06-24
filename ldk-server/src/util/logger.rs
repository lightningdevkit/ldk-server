// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs::{self, File, OpenOptions};
use std::io::{self, LineWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use log::{error, Level, LevelFilter, Log, Metadata, Record};

struct LoggerState {
	file: LineWriter<File>,
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
/// The logger does a native size/time-based rotation and retains the last 5 logs by default, if `log_max_files` is unset.
pub struct ServerLogger {
	/// The maximum log level to display
	level: LevelFilter,
	/// Groups the file and state in a single Mutex. None if file logging is disabled.
	state: Option<Mutex<LoggerState>>,
	/// Path to the log file for reopening on SIGHUP
	log_file_path: Option<PathBuf>,
}

pub struct LogConfig {
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
		level: LevelFilter, log_file_path: Option<PathBuf>, log_config: LogConfig,
	) -> Result<Arc<Self>, io::Error> {
		let state = if let Some(path) = &log_file_path {
			// Create parent directories if they don't exist
			if let Some(parent) = path.parent() {
				fs::create_dir_all(parent)?;
			}

			let file = open_log_file(path)?;

			// Check existing file metadata to persist size and age across node restarts
			let metadata = fs::metadata(path);
			let initial_size = metadata.as_ref().map(|m| m.len() as usize).unwrap_or(0);
			let created_at = metadata
				.and_then(|m| m.created().or_else(|_| m.modified()))
				.unwrap_or_else(|_| SystemTime::now());

			Some(Mutex::new(LoggerState {
				file: LineWriter::new(file),
				bytes_written: initial_size,
				created_at,
				log_max_size_bytes: log_config.log_max_size_bytes,
				log_rotation_interval_secs: log_config.log_rotation_interval_secs,
				log_max_files: log_config.log_max_files,
			}))
		} else {
			None
		};

		let logger = Arc::new(ServerLogger { level, log_file_path, state });

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
		if let (Some(state_mutex), Some(log_file_path)) = (&self.state, &self.log_file_path) {
			if let Ok(mut state) = state_mutex.lock() {
				state.file.flush()?;
				let file = open_log_file(log_file_path)?;

				// Reset size and age tracking for the new file
				let metadata = fs::metadata(log_file_path);
				state.bytes_written = metadata.as_ref().map(|m| m.len() as usize).unwrap_or(0);
				state.created_at = metadata
					.and_then(|m| m.created().or_else(|_| m.modified()))
					.unwrap_or_else(|_| SystemTime::now());

				state.file = LineWriter::new(file);
				return Ok(());
			}
			return Err(io::Error::other("Logger state mutex poisoned"));
		}
		Ok(())
	}

	/// Flushes the current file, renames it with a timestamp, opens a fresh log,
	/// and synchronously deletes older log files.
	fn rotate(
		&self, state: &mut LoggerState, system_time_now: SystemTime,
	) -> Result<(), io::Error> {
		state.file.flush()?;
		let log_file_path = if let Some(path) = self.log_file_path.as_ref() {
			path
		} else {
			return Ok(());
		};

		let now = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
		let mut new_path = log_file_path.to_path_buf().into_os_string();
		new_path.push(".");
		new_path.push(now);
		let rotated_path = PathBuf::from(new_path);

		fs::rename(log_file_path, &rotated_path)?;

		let new_file = open_log_file(log_file_path)?;
		state.file = LineWriter::new(new_file);

		// Reset our rotation triggers for the new file
		state.bytes_written = 0;
		state.created_at = system_time_now;

		Ok(())
	}
}

impl Log for ServerLogger {
	fn enabled(&self, metadata: &Metadata) -> bool {
		metadata.level() <= self.level
	}

	fn log(&self, record: &Record) {
		if self.enabled(record.metadata()) {
			let now = SystemTime::now();
			let level_str = format_level(record.level());
			let line = record.line().unwrap_or(0);

			let log_line = format!(
				"[{} {} {}:{}] {}",
				format_timestamp(now),
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

			let mut cleanup_max_files = None;

			if let Some(state_mutex) = &self.state {
				// Log to file
				let log_bytes = log_line.len() + 1;

				if let Ok(mut state) = state_mutex.lock() {
					let mut needs_rotation = false;

					if state.log_max_size_bytes > 0
						&& state.bytes_written + log_bytes > state.log_max_size_bytes
					{
						needs_rotation = true;
					} else if state.log_rotation_interval_secs > 0 {
						if let Ok(age) = now.duration_since(state.created_at) {
							if age.as_secs() > state.log_rotation_interval_secs {
								needs_rotation = true;
							}
						}
					}

					if needs_rotation {
						if let Err(e) = self.rotate(&mut state, now) {
							eprintln!("Failed to rotate log file: {e}");
						} else {
							cleanup_max_files = Some(state.log_max_files);
						}
					}

					let _ = writeln!(state.file, "{}", log_line);
					state.bytes_written += log_bytes;
				}
			}

			// Clean up old log files
			if let (Some(log_max_files), Some(log_file_path)) =
				(cleanup_max_files, &self.log_file_path)
			{
				if let Err(e) = cleanup_old_logs(log_file_path, log_max_files) {
					error!("Failed to clean up old log files: {e}");
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

fn format_timestamp(now: SystemTime) -> String {
	let date_time: chrono::DateTime<chrono::Utc> = now.into();
	date_time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
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
	let parent = log_file_path.parent().ok_or_else(|| {
		io::Error::new(io::ErrorKind::InvalidInput, "Log file path has no parent directory")
	})?;

	let file_name = log_file_path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
		io::Error::new(io::ErrorKind::InvalidInput, "Log file path has an invalid file name")
	})?;

	let rotated_prefix = format!("{}.", file_name);

	let mut entries: Vec<_> = fs::read_dir(parent)?
		.filter_map(|entry| entry.ok())
		.filter(|entry| {
			entry
				.file_name()
				.to_str()
				.and_then(|name| {
					name.strip_prefix(&rotated_prefix).map(|suffix| {
						// The timestamp format "YYYY-MM-DDTHH-MM-SSZ" is 20 chars, contains 'T', and ends with 'Z'
						suffix.len() == 20 && suffix.contains('T') && suffix.ends_with('Z')
					})
				})
				.unwrap_or(false)
		})
		.collect();

	// Sort by modification time (oldest first)
	entries.sort_by_cached_key(|e| {
		e.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::now())
	});

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
