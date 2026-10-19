//! Настройки из переменных окружения (в Docker — из файла `bot.env`).

use crate::render::RenderCfg;
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
    fn errors() {
        assert!(cfg(&[]).is_err());
        assert!(cfg(&[HOOK]).is_err());
        assert!(cfg(&[("LOLKA_WEBHOOK_URL", "http://x"), ("POKEBONK_PLAYERS", "a")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_CLAN_ID", "../etc")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("CLAN_POLL_SECS", "5")]).is_err());
        assert!(cfg(&[HOOK, ("POKEBONK_PLAYERS", "a"), ("CLAN_POLL_SECS", "x")]).is_err());
    }
}
