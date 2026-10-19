//! Минимальный логгер в stderr (его собирает `docker logs`). Без зависимостей.

use std::sync::atomic::{AtomicU8, Ordering};

static LEVEL: AtomicU8 = AtomicU8::new(2);

/// `error`, `warn`, `info` (по умолчанию) или `debug`.
pub fn init(level: &str) {
    let l = match level.to_ascii_lowercase().as_str() {
        "error" => 0,
        "warn" | "warning" => 1,
        "debug" | "trace" => 3,
        _ => 2,
    };
    LEVEL.store(l, Ordering::Relaxed);
}

pub fn enabled(level: u8) -> bool {
    level <= LEVEL.load(Ordering::Relaxed)
}

pub fn write(tag: &str, args: std::fmt::Arguments<'_>) {
    eprintln!(
        "{} {tag:<5} {args}",
        crate::util::iso8601(crate::util::now_ms())
    );
}

#[allow(unused_macros)]
macro_rules! error {
    ($($t:tt)*) => { if $crate::log::enabled(0) { $crate::log::write("ERROR", format_args!($($t)*)) } };
}
#[allow(unused_macros)]
macro_rules! warn {
    ($($t:tt)*) => { if $crate::log::enabled(1) { $crate::log::write("WARN", format_args!($($t)*)) } };
}
#[allow(unused_macros)]
macro_rules! info {
    ($($t:tt)*) => { if $crate::log::enabled(2) { $crate::log::write("INFO", format_args!($($t)*)) } };
}
#[allow(unused_macros)]
macro_rules! debug {
    ($($t:tt)*) => { if $crate::log::enabled(3) { $crate::log::write("DEBUG", format_args!($($t)*)) } };
}
