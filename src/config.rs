//! Настройки из переменных окружения (в Docker — из файла `bot.env`).

use crate::render::RenderCfg;
use crate::tz::{self, Zone};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_SPRITE: &str =
    "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/{id}.png";

/// Секрет, который не попадёт в логи даже через `{:?}`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Для каких забегов подгружать сборку (приёмы, предметы, статы).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDetails {
    Off,
    /// Только победы.
    Wins,
    /// Победы и забеги длиннее `LONG_RUN_SECS` — короткие сливы без подробностей.
    Long,
    All,
}

/// Забег от 5 минут считается достаточно долгим для подробной карточки.
pub const LONG_RUN_SECS: f64 = 300.0;

impl BuildDetails {
    /// Нужна ли сборка для забега. У нерейтинговых забегов нет id — им нельзя.
    pub fn wants(self, victory: bool, time_secs: f64, has_id: bool) -> bool {
        has_id
            && match self {
                Self::Off => false,
                Self::Wins => victory,
                Self::Long => victory || time_secs >= LONG_RUN_SECS,
                Self::All => true,
            }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub webhook_url: Secret,
    pub webhook_username: Option<String>,
    pub clan_id: Option<String>,
    pub players: Vec<String>,
    pub clan_every: Duration,
    pub sweep_every: Duration,
    pub season_every: Option<Duration>,
    pub remind_before: Duration,
    pub state_path: PathBuf,
    pub api_base: String,
    pub device_id: Option<String>,
    pub dry_run: bool,
    pub log_level: String,
    pub render: RenderCfg,
    pub build_details: BuildDetails,
    /// Канал новостей игры (отдельный вебхук); `None` — выключено.
    pub news_webhook: Option<Secret>,
    pub news_every: Duration,
    /// Канал с живым табло клана (отдельный вебхук); `None` — выключено.
    pub board_webhook: Option<Secret>,
    pub board_every: Duration,
    /// Итоги дня; `None` — выключены.
    pub digest: Option<DigestCfg>,
    /// Часовой пояс для итогов дня.
    pub zone: Zone,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DigestCfg {
    /// Во сколько публиковать, минута суток по местному времени.
    pub minute: i64,
    /// Свой вебхук; по умолчанию — основной канал.
    pub webhook: Option<Secret>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Разбор через функцию-источник, чтобы можно было тестировать без env.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let var = |key: &str| {
            get(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let webhook_url = var("LOLKA_WEBHOOK_URL").ok_or("не задан LOLKA_WEBHOOK_URL")?;
        if !webhook_url.starts_with("https://") {
            return Err("LOLKA_WEBHOOK_URL должен начинаться с https://".into());
        }
        let optional_hook = |key: &str| -> Result<Option<Secret>, String> {
            match var(key) {
                Some(url) if url.starts_with("https://") => Ok(Some(Secret::new(url))),
                Some(_) => Err(format!("{key} должен начинаться с https://")),
                None => Ok(None),
            }
        };

        let clan_id = var("POKEBONK_CLAN_ID");
        if let Some(id) = &clan_id {
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(format!("POKEBONK_CLAN_ID выглядит неправильно: {id}"));
            }
        }
        let players: Vec<String> = var("POKEBONK_PLAYERS")
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        if clan_id.is_none() && players.is_empty() {
            return Err("задайте POKEBONK_CLAN_ID и/или POKEBONK_PLAYERS".into());
        }

        let secs = |key: &str, default: u64, min: u64| -> Result<u64, String> {
            let v = match var(key) {
                Some(s) => s
                    .parse::<u64>()
                    .map_err(|_| format!("{key}: ожидается число секунд, получено «{s}»"))?,
                None => default,
            };
            if v != 0 && v < min {
                return Err(format!("{key}: минимум {min} с, чтобы не перегружать API"));
            }
            Ok(v)
        };
        let clan_every = secs("CLAN_POLL_SECS", 60, 20)?.max(20);
        let sweep_every = secs("PLAYERS_POLL_SECS", 600, 60)?.max(60);
        let season_secs = secs("SEASON_POLL_SECS", 900, 300)?;
        let remind_hours = match var("SEASON_REMIND_HOURS") {
            Some(s) => s.parse::<u64>().map_err(|_| {
                format!("SEASON_REMIND_HOURS: ожидается число часов, получено «{s}»")
            })?,
            None => 24,
        };

        let build_details = match var("BUILD_DETAILS").as_deref() {
            None | Some("long") => BuildDetails::Long,
            Some("off" | "none" | "0") => BuildDetails::Off,
            Some("wins") => BuildDetails::Wins,
            Some("all") => BuildDetails::All,
            Some(other) => {
                return Err(format!(
                    "BUILD_DETAILS: ожидается off, wins, long или all, получено «{other}»"
                ))
            }
        };

        let news_secs = secs("NEWS_POLL_SECS", 1800, 300)?.max(300);
        let board_secs = secs("BOARD_UPDATE_SECS", 300, 60)?.max(60);

        let zone_name = var("TIMEZONE").unwrap_or_else(|| "Europe/Moscow".to_string());
        let zone = Zone::parse(&zone_name).ok_or_else(|| {
            format!("TIMEZONE: неизвестный пояс «{zone_name}» (пример: Europe/Moscow, +03:00)")
        })?;
        let digest = match var("DIGEST_TIME").as_deref() {
            Some("off" | "none" | "0") => None,
            other => {
                let text = other.unwrap_or("23:00");
                let minute = tz::parse_hhmm(text).ok_or_else(|| {
                    format!("DIGEST_TIME: ожидается ЧЧ:ММ или off, получено «{text}»")
                })?;
                Some(DigestCfg {
                    minute,
                    webhook: optional_hook("LOLKA_DIGEST_WEBHOOK_URL")?,
                })
            }
        };

        let sprite_url = match get("SPRITE_URL") {
            None => Some(DEFAULT_SPRITE.to_string()),
            Some(s) if s.trim().is_empty() => None,
            Some(s) => Some(s.trim().to_string()),
        };

        Ok(Self {
            webhook_url: Secret::new(webhook_url),
            webhook_username: var("WEBHOOK_USERNAME"),
            clan_id,
            players,
            clan_every: Duration::from_secs(clan_every),
            sweep_every: Duration::from_secs(sweep_every),
            season_every: (season_secs > 0).then(|| Duration::from_secs(season_secs)),
            remind_before: Duration::from_secs(remind_hours * 3600),
            state_path: var("STATE_PATH")
                .map_or_else(|| PathBuf::from("/data/state.json"), PathBuf::from),
            api_base: var("POKEBONK_API")
                .unwrap_or_else(|| "https://api.pokebonk.com".to_string())
                .trim_end_matches('/')
                .to_string(),
            device_id: var("DEVICE_ID"),
            dry_run: var("DRY_RUN").is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes")),
            log_level: var("LOG_LEVEL").unwrap_or_else(|| "info".to_string()),
            render: RenderCfg {
                sprite_url,
                player_url: var("PLAYER_URL"),
            },
            build_details,
            news_webhook: optional_hook("LOLKA_NEWS_WEBHOOK_URL")?,
            news_every: Duration::from_secs(news_secs),
            board_webhook: optional_hook("LOLKA_BOARD_WEBHOOK_URL")?,
            board_every: Duration::from_secs(board_secs),
            digest,
            zone,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        Config::from_lookup(|k| map.get(k).cloned())
    }

    const HOOK: (&str, &str) = ("LOLKA_WEBHOOK_URL", "https://lolka.app/api/webhooks/1/abc");

    #[test]
    fn defaults() {
        let c = cfg(&[HOOK, ("POKEBONK_CLAN_ID", "a93a1cc4-c624")]).unwrap();
        assert_eq!(c.clan_every, Duration::from_secs(60));
        assert_eq!(c.sweep_every, Duration::from_secs(600));
        assert_eq!(c.season_every, Some(Duration::from_secs(900)));
        assert_eq!(c.remind_before, Duration::from_secs(24 * 3600));
        assert_eq!(c.api_base, "https://api.pokebonk.com");
        assert_eq!(c.render.sprite_url.as_deref(), Some(DEFAULT_SPRITE));
        assert!(!c.dry_run);
        assert_eq!(c.build_details, BuildDetails::Long);
        assert_eq!(c.news_webhook, None);
        assert_eq!(c.board_webhook, None);
        assert_eq!(c.board_every, Duration::from_secs(300));
        assert_eq!(
            c.digest,
            Some(DigestCfg {
                minute: 23 * 60,
                webhook: None
            })
        );
        assert_eq!(c.zone, Zone::Fixed(180));
        assert!(!format!("{c:?}").contains("abc"), "секрет попал в Debug");
    }

    #[test]
    fn players_list_and_overrides() {
        let c = cfg(&[
            HOOK,
            ("POKEBONK_PLAYERS", " 3x1ee, Timesan ,,"),
            ("SEASON_POLL_SECS", "0"),
            ("SPRITE_URL", ""),
            ("DRY_RUN", "true"),
        ])
        .unwrap();
        assert_eq!(c.players, vec!["3x1ee", "Timesan"]);
        assert_eq!(c.season_every, None);
        assert_eq!(c.render.sprite_url, None);
        assert!(c.dry_run);
    }

    #[test]
    fn channels_and_digest() {
        let c = cfg(&[
            HOOK,
            ("POKEBONK_PLAYERS", "a"),
            (
                "LOLKA_NEWS_WEBHOOK_URL",
                "https://lolka.app/api/webhooks/2/n",
            ),
            (
                "LOLKA_BOARD_WEBHOOK_URL",
                "https://lolka.app/api/webhooks/3/b",
            ),
            ("DIGEST_TIME", "21:30"),
            ("TIMEZONE", "Europe/Warsaw"),
        ])
        .unwrap();
        assert!(c.news_webhook.is_some());
        assert!(c.board_webhook.is_some());
        assert_eq!(c.digest.unwrap().minute, 21 * 60 + 30);
        assert_eq!(c.zone, Zone::Eu(60));

        let off = cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("DIGEST_TIME", "off")]).unwrap();
        assert_eq!(off.digest, None);
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("DIGEST_TIME", "25:00")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("TIMEZONE", "Mars")]).is_err());
        assert!(cfg(&[
            HOOK,
            ("POKEBONK_PLAYERS", "a"),
            ("LOLKA_NEWS_WEBHOOK_URL", "http://x")
        ])
        .is_err());
    }

    #[test]
    fn build_details() {
        let c = cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("BUILD_DETAILS", "wins")]).unwrap();
        assert_eq!(c.build_details, BuildDetails::Wins);
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("BUILD_DETAILS", "x")]).is_err());

        let long = BuildDetails::Long;
        assert!(long.wants(true, 60.0, true));
        assert!(long.wants(false, 301.0, true));
        assert!(!long.wants(false, 100.0, true));
        assert!(!long.wants(true, 900.0, false), "без id сборку не получить");
        assert!(!BuildDetails::Off.wants(true, 900.0, true));
        assert!(BuildDetails::All.wants(false, 1.0, true));
    }

    #[test]
    fn errors() {
        assert!(cfg(&[]).is_err());
        assert!(cfg(&[HOOK]).is_err());
        assert!(cfg(&[("LOLKA_WEBHOOK_URL", "http://x"), ("POKEBONK_PLAYERS", "a")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_CLAN_ID", "../etc")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("CLAN_POLL_SECS", "5")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("CLAN_POLL_SECS", "x")]).is_err());
    }
}
