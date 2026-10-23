//! Мелкие помощники: время, форматирование чисел, URL.

use std::time::{SystemTime, UNIX_EPOCH};

/// Текущее время в миллисекундах Unix (как `date` в API Pokebonk).
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Миллисекунды Unix -> ISO 8601 в UTC, например `2026-10-19T03:27:50.813Z`.
pub fn iso8601(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// (год, месяц, день) -> дни от 1970-01-01. Алгоритм Говарда Хиннанта.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Дата по-русски: `23 октября`.
pub fn fmt_date_ru(month: i64, day: i64) -> String {
    const MONTHS: [&str; 12] = [
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ];
    let name = usize::try_from(month - 1)
        .ok()
        .and_then(|i| MONTHS.get(i))
        .copied()
        .unwrap_or("?");
    format!("{day} {name}")
}

/// Дни от 1970-01-01 -> (год, месяц, день). Алгоритм Говарда Хиннанта.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Длительность забега: `8:21`, `32:27`, `1:05:09`.
pub fn fmt_duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "—".to_string();
    }
    // Время забега всегда меньше суток, округление до секунд безопасно.
    #[allow(clippy::cast_possible_truncation)]
    let total = secs.round() as i64;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Сколько осталось до события: `2 д 5 ч`, `5 ч 12 мин`, `12 мин`.
pub fn fmt_until(ms: i64) -> String {
    let mins = (ms.max(0) + 59_999) / 60_000;
    let (d, h, m) = (mins / 1440, mins % 1440 / 60, mins % 60);
    match (d, h) {
        (0, 0) => format!("{m} мин"),
        (0, _) if m > 0 => format!("{h} ч {m} мин"),
        (0, _) => format!("{h} ч"),
        (_, 0) => format!("{d} д"),
        _ => format!("{d} д {h} ч"),
    }
}

/// Разделение тысяч неразрывным пробелом: `28 077`.
pub fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{a0}');
        }
        out.push(ch);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

/// Короткая запись больших чисел, как в игре: `950`, `32.2K`, `238M`, `1.2B`.
pub fn compact(n: i64) -> String {
    // Точность до десятых здесь важнее точного значения.
    #[allow(clippy::cast_precision_loss)]
    let v = n as f64;
    let (value, suffix) = match n.unsigned_abs() {
        0..=999 => return n.to_string(),
        1_000..=999_999 => (v / 1e3, "K"),
        1_000_000..=999_999_999 => (v / 1e6, "M"),
        _ => (v / 1e9, "B"),
    };
    if value.abs() >= 99.95 {
        format!("{value:.0}{suffix}")
    } else {
        format!("{value:.1}{suffix}")
    }
}

/// Множитель в проценты: `3.08` -> `308%`, `0.27` -> `27%`.
pub fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

/// Число без лишних нулей: `4` -> `4`, `4.91` -> `4.9`, `174.28` -> `174`.
pub fn num(x: f64) -> String {
    if x.abs() >= 100.0 || (x - x.round()).abs() < 0.05 {
        format!("{x:.0}")
    } else {
        format!("{x:.1}")
    }
}

/// Процентное кодирование для подстановки в URL.
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Обрезает строку до `max` символов, добавляя многоточие.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Случайный идентификатор устройства: 32 hex-символа, как у браузера.
/// Это не секрет, поэтому хватает стандартной библиотеки.
pub fn random_hex32() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut out = String::with_capacity(32);
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_i64(now_ms());
        h.write_u32(std::process::id());
        h.write_u64(i);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        // Время из ответа вебхука Lolka в переписке.
        assert_eq!(iso8601(1_792_368_000_000), "2026-10-19T00:00:00.000Z");
        assert_eq!(iso8601(1_792_540_800_000), "2026-10-21T00:00:00.000Z");
        assert_eq!(iso8601(951_782_400_123), "2000-02-29T00:00:00.123Z");
    }

    #[test]
    fn civil_roundtrip() {
        for days in [-1000, 0, 59, 10_957, 20_745, 30_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(days_from_civil(2026, 10, 19), 20_745);
        assert_eq!(fmt_date_ru(10, 23), "23 октября");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(1947.677), "32:28");
        assert_eq!(fmt_duration(98.0), "1:38");
        assert_eq!(fmt_duration(3909.0), "1:05:09");
        assert_eq!(fmt_duration(f64::NAN), "—");
    }

    #[test]
    fn until() {
        assert_eq!(fmt_until(12 * 60_000), "12 мин");
        assert_eq!(fmt_until((5 * 60 + 12) * 60_000), "5 ч 12 мин");
        assert_eq!(fmt_until(3 * 3_600_000), "3 ч");
        assert_eq!(fmt_until((2 * 24 + 5) * 3_600_000), "2 д 5 ч");
        assert_eq!(fmt_until(48 * 3_600_000), "2 д");
        assert_eq!(fmt_until(-5), "0 мин");
    }

    #[test]
    fn numbers_and_text() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(28_077), "28\u{a0}077");
        assert_eq!(thousands(-1_234_567), "-1\u{a0}234\u{a0}567");
        assert_eq!(compact(950), "950");
        assert_eq!(compact(32_200), "32.2K");
        assert_eq!(compact(2_829_537), "2.8M");
        assert_eq!(compact(116_694_510), "117M");
        assert_eq!(compact(237_565_030), "238M");
        assert_eq!(compact(1_234_000_000), "1.2B");
        assert_eq!(compact(999_999), "1000K");
        assert_eq!(pct(3.08), "308%");
        assert_eq!(pct(0.27), "27%");
        assert_eq!(num(4.0), "4");
        assert_eq!(num(4.91), "4.9");
        assert_eq!(num(174.28), "174");
        assert_eq!(url_encode("Max Verstappen"), "Max%20Verstappen");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        let id = random_hex32();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
