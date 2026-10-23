//! Часовые пояса без внешних библиотек: в образе `scratch` нет базы tzdata,
//! поэтому поддерживаются фиксированные смещения и правило летнего времени ЕС
//! (последнее воскресенье марта и октября, 01:00 UTC).

use crate::util;

const MIN_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// Постоянное смещение от UTC в минутах.
    Fixed(i64),
    /// Европа с летним временем: смещение зимой в минутах (+60 летом).
    Eu(i64),
}

impl Zone {
    /// `Europe/Moscow`, `Europe/Warsaw`, `UTC`, `+03:00`, `UTC+3`, `-05:30`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let fixed = |h: i64| Some(Self::Fixed(h * 60));
        match s {
            "UTC" | "GMT" | "Z" | "Etc/UTC" => fixed(0),
            "Europe/Kaliningrad" => fixed(2),
            "Europe/Moscow" | "MSK" | "Europe/Simferopol" | "Europe/Minsk" | "Europe/Volgograd"
            | "Europe/Kirov" | "Europe/Istanbul" => fixed(3),
            "Europe/Samara" | "Europe/Saratov" | "Europe/Ulyanovsk" | "Europe/Astrakhan"
            | "Asia/Tbilisi" | "Asia/Baku" | "Asia/Yerevan" | "Asia/Dubai" => fixed(4),
            "Asia/Yekaterinburg" | "Asia/Tashkent" | "Asia/Almaty" => fixed(5),
            "Asia/Omsk" | "Asia/Bishkek" => fixed(6),
            "Asia/Novosibirsk" | "Asia/Krasnoyarsk" | "Asia/Barnaul" | "Asia/Tomsk"
            | "Asia/Novokuznetsk" => fixed(7),
            "Asia/Irkutsk" => fixed(8),
            "Asia/Yakutsk" | "Asia/Chita" => fixed(9),
            "Asia/Vladivostok" => fixed(10),
            "Asia/Magadan" | "Asia/Sakhalin" => fixed(11),
            "Asia/Kamchatka" => fixed(12),
            "Europe/London" | "Europe/Lisbon" | "Europe/Dublin" | "WET" => Some(Self::Eu(0)),
            "Europe/Warsaw" | "Europe/Berlin" | "Europe/Paris" | "Europe/Madrid"
            | "Europe/Rome" | "Europe/Prague" | "Europe/Vienna" | "Europe/Amsterdam"
            | "Europe/Brussels" | "Europe/Stockholm" | "Europe/Oslo" | "Europe/Copenhagen"
            | "Europe/Budapest" | "Europe/Belgrade" | "Europe/Zurich" | "CET" => Some(Self::Eu(60)),
            "Europe/Kyiv" | "Europe/Kiev" | "Europe/Helsinki" | "Europe/Riga"
            | "Europe/Tallinn" | "Europe/Vilnius" | "Europe/Sofia" | "Europe/Bucharest"
            | "Europe/Athens" | "EET" => Some(Self::Eu(120)),
            other => parse_offset(other).map(Self::Fixed),
        }
    }

    /// Смещение от UTC в минутах в момент `utc_ms`.
    pub fn offset_min(self, utc_ms: i64) -> i64 {
        match self {
            Self::Fixed(m) => m,
            Self::Eu(m) if eu_summer(utc_ms) => m + 60,
            Self::Eu(m) => m,
        }
    }

    /// Местные (год, месяц, день, минута суток).
    pub fn local(self, utc_ms: i64) -> (i64, i64, i64, i64) {
        let local = utc_ms + self.offset_min(utc_ms) * MIN_MS;
        let (y, m, d) = util::civil_from_days(local.div_euclid(DAY_MS));
        (y, m, d, local.rem_euclid(DAY_MS) / MIN_MS)
    }

    /// Ближайший после `after_ms` момент, когда местное время равно `minute_of_day`.
    pub fn next_at(self, after_ms: i64, minute_of_day: i64) -> i64 {
        let offset = self.offset_min(after_ms) * MIN_MS;
        let local_midnight = (after_ms + offset).div_euclid(DAY_MS) * DAY_MS;
        let mut day = 0;
        loop {
            let local = local_midnight + day * DAY_MS + minute_of_day * MIN_MS;
            // Смещение берём для примерного момента: переход на летнее время
            // бывает ночью, а итоги — вечером.
            let utc = local - self.offset_min(local - offset) * MIN_MS;
            if utc > after_ms {
                return utc;
            }
            day += 1;
        }
    }
}

/// `+03:00`, `-0530`, `+3`, `UTC+3` -> минуты.
fn parse_offset(s: &str) -> Option<i64> {
    let s = s
        .strip_prefix("UTC")
        .or_else(|| s.strip_prefix("GMT"))
        .unwrap_or(s);
    let (sign, rest) = match s.chars().next()? {
        '+' => (1, &s[1..]),
        '-' => (-1, &s[1..]),
        _ => return None,
    };
    let (h, m) = match rest.split_once(':') {
        Some((h, m)) => (h, m),
        None if rest.len() == 4 => rest.split_at(2),
        None => (rest, "0"),
    };
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    if !(0..=14).contains(&h) || !(0..60).contains(&m) {
        return None;
    }
    Some(sign * (h * 60 + m))
}

/// Действует ли летнее время ЕС в момент `utc_ms`.
fn eu_summer(utc_ms: i64) -> bool {
    let (year, _, _) = util::civil_from_days(utc_ms.div_euclid(DAY_MS));
    let start = last_sunday(year, 3) * DAY_MS + HOUR_MS;
    let end = last_sunday(year, 10) * DAY_MS + HOUR_MS;
    (start..end).contains(&utc_ms)
}

/// День (от 1970-01-01) последнего воскресенья месяца.
fn last_sunday(year: i64, month: i64) -> i64 {
    let last_day = util::days_from_civil(year, month + 1, 1) - 1;
    // 1970-01-01 — четверг; 0 = воскресенье.
    let weekday = (last_day + 4).rem_euclid(7);
    last_day - weekday
}

/// `23:00` -> минута суток.
pub fn parse_hhmm(s: &str) -> Option<i64> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i64, mo: i64, d: i64, h: i64, mi: i64) -> i64 {
        util::days_from_civil(y, mo, d) * DAY_MS + h * HOUR_MS + mi * MIN_MS
    }

    #[test]
    fn parsing() {
        assert_eq!(Zone::parse("Europe/Moscow"), Some(Zone::Fixed(180)));
        assert_eq!(Zone::parse("Europe/Warsaw"), Some(Zone::Eu(60)));
        assert_eq!(Zone::parse("+03:00"), Some(Zone::Fixed(180)));
        assert_eq!(Zone::parse("UTC+5"), Some(Zone::Fixed(300)));
        assert_eq!(Zone::parse("-0530"), Some(Zone::Fixed(-330)));
        assert_eq!(Zone::parse("Mars/Olympus"), None);
        assert_eq!(Zone::parse("+25"), None);
        assert_eq!(parse_hhmm("23:00"), Some(1380));
        assert_eq!(parse_hhmm("7:05"), Some(425));
        assert_eq!(parse_hhmm("24:00"), None);
    }

    #[test]
    fn eu_daylight_saving_2026() {
        // В 2026 году: 29 марта и 25 октября, 01:00 UTC.
        let warsaw = Zone::Eu(60);
        assert_eq!(warsaw.offset_min(utc(2026, 3, 29, 0, 59)), 60);
        assert_eq!(warsaw.offset_min(utc(2026, 3, 29, 1, 0)), 120);
        assert_eq!(warsaw.offset_min(utc(2026, 10, 23, 12, 0)), 120);
        assert_eq!(warsaw.offset_min(utc(2026, 10, 25, 0, 59)), 120);
        assert_eq!(warsaw.offset_min(utc(2026, 10, 25, 1, 0)), 60);
    }

    #[test]
    fn local_time_and_next_digest() {
        let msk = Zone::Fixed(180);
        // 23 октября 2026, 10:00 UTC = 13:00 МСК.
        let now = utc(2026, 10, 23, 10, 0);
        assert_eq!(msk.local(now), (2026, 10, 23, 13 * 60));
        // Итоги в 23:00 МСК = 20:00 UTC того же дня.
        assert_eq!(msk.next_at(now, 23 * 60), utc(2026, 10, 23, 20, 0));
        // Уже после 23:00 — следующий день.
        assert_eq!(
            msk.next_at(utc(2026, 10, 23, 20, 0), 23 * 60),
            utc(2026, 10, 24, 20, 0)
        );

        // Варшава до и после перехода на зимнее время.
        let warsaw = Zone::Eu(60);
        assert_eq!(
            warsaw.next_at(utc(2026, 10, 24, 12, 0), 23 * 60),
            utc(2026, 10, 24, 21, 0)
        );
        assert_eq!(
            warsaw.next_at(utc(2026, 10, 25, 12, 0), 23 * 60),
            utc(2026, 10, 25, 22, 0)
        );
    }
}
