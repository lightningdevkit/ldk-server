// This file is Copyright its original authors, visible in version control
// history.
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// A logger implementation that writes logs to both stdout/stderr and a file.
///
/// Mirrors the daemon's `ServerLogger` so operators see consistent output
/// across the two processes.
///
/// All log messages follow the format:
/// `[TIMESTAMP LEVEL TARGET:LINE] MESSAGE`
pub struct GatewayLogger {
	level: LevelFilter,
	file: Mutex<File>,
	log_file_path: PathBuf,
}

impl GatewayLogger {
	/// Initializes the global logger with the specified level and file path.
	///
	/// Should be called once at application startup.
	pub fn init(level: LevelFilter, log_file_path: &Path) -> Result<Arc<Self>, io::Error> {
		if let Some(parent) = log_file_path.parent() {
			fs::create_dir_all(parent)?;
		}

		let file = open_log_file(log_file_path)?;

		let logger = Arc::new(GatewayLogger {
			level,
			file: Mutex::new(file),
			log_file_path: log_file_path.to_path_buf(),
		});

		log::set_boxed_logger(Box::new(LoggerWrapper(Arc::clone(&logger))))
			.map_err(io::Error::other)?;
		log::set_max_level(level);
		Ok(logger)
	}

	/// Reopens the log file. Called on SIGHUP for log rotation.
	pub fn reopen(&self) -> Result<(), io::Error> {
		let new_file = open_log_file(&self.log_file_path)?;
		match self.file.lock() {
			Ok(mut file) => {
				file.flush()?;
				*file = new_file;
				Ok(())
			},
			Err(e) => Err(io::Error::other(format!("Failed to acquire lock: {e}"))),
		}
	}
}

impl Log for GatewayLogger {
	fn enabled(&self, metadata: &Metadata) -> bool {
		metadata.level() <= self.level
	}

	fn log(&self, record: &Record) {
		if self.enabled(record.metadata()) {
			let level_str = format_level(record.level());
			let line = record.line().unwrap_or(0);

			let _ = match record.level() {
				Level::Error => writeln!(
					io::stderr(),
					"[{} {} {}:{}] {}",
					format_timestamp(),
					level_str,
					record.target(),
					line,
					record.args()
				),
				_ => writeln!(
					io::stdout(),
					"[{} {} {}:{}] {}",
					format_timestamp(),
					level_str,
					record.target(),
					line,
					record.args()
				),
			};

			if let Ok(mut file) = self.file.lock() {
				let _ = writeln!(
					file,
					"[{} {} {}:{}] {}",
					format_timestamp(),
					level_str,
					record.target(),
					line,
					record.args()
				);
			}
		}
	}

	fn flush(&self) {
		let _ = io::stdout().flush();
		let _ = io::stderr().flush();
		if let Ok(mut file) = self.file.lock() {
			let _ = file.flush();
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

struct LoggerWrapper(Arc<GatewayLogger>);

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
