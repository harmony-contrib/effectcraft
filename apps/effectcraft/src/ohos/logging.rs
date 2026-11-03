//! HiLog and bounded, rotating logs in the Ability's preferences directory.

use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock, PoisonError},
};

struct FileLog {
    file: File,
    bytes: u64,
}

struct Logger {
    file: Mutex<Option<FileLog>>,
    filters: Vec<(String, log::LevelFilter)>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        let level = self.filters.iter().filter(|(prefix, _)| metadata.target().starts_with(prefix)).max_by_key(|(prefix, _)| prefix.len());
        level.is_some_and(|(_, level)| metadata.level() <= *level)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
        let thread = std::thread::current();
        let line = format!(
            "{} {:<5} [{}] {}: {}\n",
            now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            record.level(),
            thread.name().unwrap_or("?"),
            record.target(),
            record.args()
        );
        let message = line.trim_end().replace('\0', "");
        match record.level() {
            log::Level::Error => ohos_hilog_binding::error(message),
            log::Level::Warn => ohos_hilog_binding::warn(message),
            log::Level::Info => ohos_hilog_binding::info(message),
            log::Level::Debug | log::Level::Trace => ohos_hilog_binding::debug(message),
        }
        let mut sink = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(sink) = sink.as_mut() {
            let bytes = u64::try_from(line.len()).unwrap_or(u64::MAX);
            if sink.bytes.saturating_add(bytes) <= effectcraft_engine::logging::MAX_FILE_BYTES {
                if let Err(error) = sink.file.write_all(line.as_bytes()) {
                    ohos_hilog_binding::warn(format!("EffectCraft file logging stopped: {error}"));
                    sink.bytes = effectcraft_engine::logging::MAX_FILE_BYTES;
                } else {
                    sink.bytes = sink.bytes.saturating_add(bytes);
                }
            }
        }
    }

    fn flush(&self) {
        if let Some(sink) = self.file.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            let _ = sink.file.flush();
        }
    }
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn install() -> Result<(), String> {
    let logger = LOGGER.get_or_init(|| {
        let spec = std::env::var("RUST_LOG").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| effectcraft_engine::logging::DEFAULT_FILTER.into());
        let filters = spec
            .split(',')
            .filter_map(|directive| {
                let directive = directive.trim();
                match directive.split_once('=') {
                    Some((target, level)) => Some((target.trim().to_owned(), level.trim().parse().ok()?)),
                    None => Some((String::new(), directive.parse().ok()?)),
                }
            })
            .collect();
        Logger { file: Mutex::new(None), filters }
    });
    log::set_logger(logger).map_err(|error| format!("Cannot install OpenHarmony logger: {error}"))?;
    log::set_max_level(logger.filters.iter().map(|(_, level)| *level).max().unwrap_or(log::LevelFilter::Warn));
    effectcraft_engine::logging::install_panic_hook();
    Ok(())
}

pub fn attach(directory: &Path) {
    let attach = || -> std::io::Result<FileLog> {
        let directory = directory.join("logs");
        std::fs::create_dir_all(&directory)?;
        let path = |n| if n == 0 { directory.join("effectcraft.log") } else { directory.join(format!("effectcraft.{n}.log")) };
        for n in (1..=effectcraft_engine::logging::KEEP).rev() {
            match std::fs::rename(path(n - 1), path(n)) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
        }
        Ok(FileLog { file: File::create(path(0))?, bytes: 0 })
    };
    match attach() {
        Ok(sink) => {
            if let Some(logger) = LOGGER.get() {
                *logger.file.lock().unwrap_or_else(PoisonError::into_inner) = Some(sink);
            }
        }
        Err(error) => log::warn!("Cannot open session log: {error}; logs remain available in HiLog"),
    }
}
