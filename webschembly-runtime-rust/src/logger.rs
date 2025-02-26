use crate::env;
use log::{Log, Metadata, Record};

// https://gitlab.com/limira-rs/wasm-logger

pub struct WasmLogger;

impl Log for WasmLogger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let s = format!(
                "[{}] {}:{} {}",
                record.level(),
                record.file().unwrap_or_else(|| record.target()),
                record
                    .line()
                    .map_or_else(|| "[Unknown]".to_string(), |line| line.to_string()),
                record.args(),
            );
            str_log(&s);
        }
    }

    fn flush(&self) {}
}

fn str_log(s: &str) {
    let buf = s.as_bytes();
    unsafe {
        env::js_webschembly_log(buf.as_ptr() as i32, buf.len() as i32);
    }
}
