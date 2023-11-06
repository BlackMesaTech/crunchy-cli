use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use log::{
    info, set_boxed_logger, set_max_level, Level, LevelFilter, Log, Metadata, Record,
    SetLoggerError,
};
use std::io::{stdout, Write};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct ProgressHandler {
    pub(crate) stopped: bool,
}

impl Drop for ProgressHandler {
    fn drop(&mut self) {
        if !self.stopped {
            info!(target: "progress_end", "")
        }
    }
}

impl ProgressHandler {
    pub(crate) fn stop<S: AsRef<str>>(mut self, msg: S) {
        self.stopped = true;
        info!(target: "progress_end", "{}", msg.as_ref())
    }
}

macro_rules! progress {
    ($($arg:tt)+) => {
        {
            log::info!(target: "progress", $($arg)+);
            $crate::utils::log::ProgressHandler{stopped: false}
        }
    }
}
pub(crate) use progress;

macro_rules! progress_pause {
    () => {
        {
            log::info!(target: "progress_pause", "")
        }
    }
}
pub(crate) use progress_pause;

macro_rules! tab_info {
    ($($arg:tt)+) => {
        if log::max_level() == log::LevelFilter::Debug {
            info!($($arg)+)
        } else {
            info!("\t{}", format!($($arg)+))
        }
    }
}
pub(crate) use tab_info;

pub struct CliLogger {
    level: LevelFilter,
    progress: Mutex<Option<ProgressBar>>,
}

impl Log for CliLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata())
            || (record.target() != "progress"
                && record.target() != "progress_pause"
                && record.target() != "progress_end"
                && !record.target().starts_with("crunchy_cli"))
        {
            return;
        }

        if self.level >= LevelFilter::Debug {
            self.extended(record);
            return;
        }

        match record.target() {
            "progress" => self.progress(record, false),
            "progress_pause" => {
                let progress = self.progress.lock().unwrap();
                if let Some(p) = &*progress {
                    p.set_draw_target(if p.is_hidden() {
                        ProgressDrawTarget::stdout()
                    } else {
                        ProgressDrawTarget::hidden()
                    })
                }
            }
            "progress_end" => self.progress(record, true),
            _ => {
                if self.progress.lock().unwrap().is_some() {
                    self.progress(record, false)
                } else if record.level() > Level::Warn {
                    self.normal(record)
                } else {
                    self.error(record)
                }
            }
        }
    }

    fn flush(&self) {
        let _ = stdout().flush();
    }
}

impl CliLogger {
    pub fn new(level: LevelFilter) -> Self {
        Self {
            level,
            progress: Mutex::new(None),
        }
    }

    pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
        set_max_level(level);
        set_boxed_logger(Box::new(CliLogger::new(level)))
    }

    fn extended(&self, record: &Record) {
        println!(
            "[{}] {}  {} ({}) {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            // replace the 'progress' prefix if this function is invoked via 'progress!'
            record
                .target()
                .replacen("crunchy_cli_core", "crunchy_cli", 1)
                .replacen("progress_end", "crunchy_cli", 1)
                .replacen("progress", "crunchy_cli", 1),
            format!("{:?}", thread::current().id())
                .replace("ThreadId(", "")
                .replace(')', ""),
            record.args()
        )
    }

    fn normal(&self, record: &Record) {
        println!(":: {}", record.args())
    }

    fn error(&self, record: &Record) {
        eprintln!(":: {}", record.args())
    }

    fn progress(&self, record: &Record, stop: bool) {
        let mut progress = self.progress.lock().unwrap();

        let msg = format!("{}", record.args());
        if stop && progress.is_some() {
            if msg.is_empty() {
                progress.take().unwrap().finish()
            } else {
                progress.take().unwrap().finish_with_message(msg)
            }
        } else if let Some(p) = &*progress {
            p.println(format!(":: → {}", msg))
        } else {
            #[cfg(not(windows))]
            let finish_str = "✔";
            #[cfg(windows)]
            // windows does not support all unicode characters by default in their consoles, so
            // we're using this (square root) symbol instead. microsoft.
            let finish_str = "√";

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template(":: {spinner} {msg}")
                    .unwrap()
                    .tick_strings(&["—", "\\", "|", "/", finish_str]),
            );
            pb.set_draw_target(ProgressDrawTarget::stdout());
            pb.enable_steady_tick(Duration::from_millis(200));
            pb.set_message(msg);
            *progress = Some(pb)
        }
    }
}
