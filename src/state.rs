//! Состояние бота в JSON-файле: что уже видели, гостевой токен, неотправленные
//! сообщения. Благодаря нему после перезапуска бот не повторяет старые забеги.

use crate::model::{ClanSnap, Message, PlayerSnap, SeasonSnap};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Сколько неотправленных сообщений хранить, если Lolka недоступна.
pub const MAX_OUTBOX: usize = 50;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub device_id: String,
    pub guest_token: Option<String>,
    /// Снимки игроков по id.
    pub players: BTreeMap<String, PlayerSnap>,
    /// Ник из POKEBONK_PLAYERS (в нижнем регистре) -> id игрока.
    pub names: BTreeMap<String, String>,
    pub clan: Option<ClanSnap>,
    pub season: SeasonSnap,
    /// Сообщения, которые не удалось отправить; отправятся в следующий раз.
    pub outbox: Vec<Message>,
}

impl State {
    /// Загружает состояние. Нет файла — чистый старт. Испорченный файл
    /// откладывается в сторону, чтобы бот не падал в цикле перезапусков.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!("файла состояния нет, начинаю с нуля: {}", path.display());
                return Ok(Self::default());
            }
            Err(e) => return Err(e).with_context(|| format!("не читается {}", path.display())),
        };
        match serde_json::from_str(&text) {
            Ok(state) => Ok(state),
            Err(e) => {
                let backup = path.with_extension("broken.json");
                warn!(
                    "файл состояния испорчен ({e}), сохраняю копию в {} и начинаю с нуля",
                    backup.display()
                );
                fs::rename(path, &backup).ok();
                Ok(Self::default())
            }
        }
    }

    /// Атомарная запись: сначала во временный файл, потом переименование.
    pub fn save_text(path: &Path, text: &str) -> Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).with_context(|| format!("не создаётся {}", dir.display()))?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, text).with_context(|| format!("не пишется {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("не переименовывается в {}", path.display()))
    }

    pub fn to_text(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_recovery() {
        let dir = std::env::temp_dir().join(format!("pbl-test-{}", crate::util::random_hex32()));
        let path = dir.join("state.json");

        assert_eq!(State::load(&path).unwrap(), State::default());

        let mut s = State {
            device_id: "dev".into(),
            ..State::default()
        };
        s.players.insert("p1".into(), PlayerSnap::default());
        State::save_text(&path, &s.to_text().unwrap()).unwrap();
        assert_eq!(State::load(&path).unwrap(), s);

        fs::write(&path, "{ not json").unwrap();
        assert_eq!(State::load(&path).unwrap(), State::default());
        assert!(path.with_extension("broken.json").exists());

        fs::remove_dir_all(dir).ok();
    }
}
