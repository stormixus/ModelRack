#![windows_subsystem = "windows"]

mod db;
mod fonts;
mod macos;
mod scanner;
mod strings;
mod thumbnail_cache;
#[cfg(test)]
mod utils;
mod view_model;

mod slint_shell;

struct QuietLogger;

impl log::Log for QuietLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let target = metadata.target();
        !(target.starts_with("icu_") || target.starts_with("parley") || target.starts_with("slint"))
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: QuietLogger = QuietLogger;

#[cfg(unix)]
fn spawn_stderr_filter() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::io::FromRawFd;

    unsafe {
        let original_stderr_fd = libc::dup(libc::STDERR_FILENO);
        if original_stderr_fd < 0 {
            return;
        }
        let mut original_stderr = std::fs::File::from_raw_fd(original_stderr_fd);

        let mut fds = [0; 2];
        if libc::pipe(fds.as_mut_ptr()) < 0 {
            return;
        }

        if libc::dup2(fds[1], libc::STDERR_FILENO) < 0 {
            return;
        }
        libc::close(fds[1]);

        let read_fd = fds[0];
        std::thread::spawn(move || {
            let pipe_read_file = std::fs::File::from_raw_fd(read_fd);
            let reader = BufReader::new(pipe_read_file);

            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    // Filter out the noisy ICU4X segmentation error message
                    if line.contains("No segmentation model for language")
                        || line.contains("No segmentation model for language: ja")
                    {
                        continue;
                    }
                    let _ = writeln!(original_stderr, "{}", line);
                } else {
                    break;
                }
            }
        });
    }
}

#[cfg(not(unix))]
fn spawn_stderr_filter() {}

fn main() -> Result<(), slint::PlatformError> {
    spawn_stderr_filter();
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Warn));
    slint_shell::run()
}
