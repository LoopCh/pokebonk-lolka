//! Клиент API Pokebonk.
//!
//! * `/players` — публичный, без токена.
//! * `/clans/{id}` и `/leaderboard` — нужна сессия; хватает гостевой из
//!   `POST /auth/guest`. Протухший токен заменяется новым автоматически.

use crate::model::{ApiError, Clan, GuestAuth, Leaderboard, Player};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use std::time::{Duration, Instant};

/// Не создавать гостей чаще, чем раз в 10 минут, даже если что-то пошло не так.
const GUEST_COOLDOWN: Duration = Duration::from_secs(600);

pub struct Api {
    http: Client,
    base: String,
    device_id: String,
    token: Option<String>,
    last_guest: Option<Instant>,
}

impl Api {
    pub fn new(http: Client, base: String, device_id: String, token: Option<String>) -> Self {
        Self {
            http,
            base,
            device_id,
            token,
            last_guest: None,
        }
    }

    /// Текущий гостевой токен (сохраняется в состоянии).
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub async fn player_by_id(&mut self, id: &str) -> Result<Player> {
        self.get("/players", &[("id", id)], false).await
    }

    pub async fn player_by_name(&mut self, name: &str) -> Result<Player> {
        self.get("/players", &[("name", name)], false).await
    }

    pub async fn clan(&mut self, id: &str) -> Result<Clan> {
        self.get(&format!("/clans/{id}"), &[], true).await
    }

    pub async fn leaderboard(&mut self) -> Result<Leaderboard> {
        self.get("/leaderboard", &[("mode", "kills")], true).await
    }

    async fn get<T: DeserializeOwned>(
        &mut self,
        path: &str,
        query: &[(&str, &str)],
        auth: bool,
    ) -> Result<T> {
        for attempt in 0..2 {
            if auth && self.token.is_none() {
                self.new_guest().await?;
            }
            let mut req = self
                .http
                .get(format!("{}{path}", self.base))
                .query(query)
                .header("X-Device", &self.device_id)
                .header("Accept", "application/json");
            if auth {
                if let Some(token) = &self.token {
                    req = req.bearer_auth(token);
                }
            }

            let resp = req
                .send()
                .await
                .map_err(|e| anyhow!("{path}: {}", e.without_url()))?;
            let status = resp.status();
            let body = resp
                .text()
                .await
                .map_err(|e| anyhow!("{path}: {}", e.without_url()))?;

            // API может вернуть {"error": …} даже с кодом 200.
            let api_error = serde_json::from_str::<ApiError>(&body)
                .ok()
                .map(|e| e.error);
            if status.is_success() && api_error.is_none() {
                return serde_json::from_str(&body)
                    .with_context(|| format!("{path}: неожиданный ответ: {}", snippet(&body)));
            }

            let message = api_error.unwrap_or_else(|| snippet(&body));
            if auth && attempt == 0 && is_session_error(status, &message) {
                warn!("pokebonk: гостевая сессия истекла ({message}), получаю новую");
                self.token = None;
                continue;
            }
            bail!("{path}: HTTP {}: {message}", status.as_u16());
        }
        bail!("{path}: сессия не принимается даже после обновления")
    }

    async fn new_guest(&mut self) -> Result<()> {
        if let Some(t) = self.last_guest {
            if t.elapsed() < GUEST_COOLDOWN {
                bail!(
                    "нужна новая гостевая сессия, но прошлая создана {} с назад; подожду",
                    t.elapsed().as_secs()
                );
            }
        }
        self.last_guest = Some(Instant::now());

        let resp = self
            .http
            .post(format!("{}/auth/guest", self.base))
            .header("X-Device", &self.device_id)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| anyhow!("/auth/guest: {}", e.without_url()))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| anyhow!("/auth/guest: {}", e.without_url()))?;
        if !status.is_success() {
            bail!("/auth/guest: HTTP {}: {}", status.as_u16(), snippet(&body));
        }
        let guest: GuestAuth = serde_json::from_str(&body)
            .with_context(|| format!("/auth/guest: неожиданный ответ: {}", snippet(&body)))?;
        if guest.token.is_empty() {
            bail!("/auth/guest: в ответе нет токена");
        }
        info!("pokebonk: новая гостевая сессия «{}»", guest.name);
        self.token = Some(guest.token);
        Ok(())
    }
}

/// Похоже ли на «сессия истекла». Сервер отвечает по-испански:
/// «Tu sesión ha caducado. Vuelve a entrar.»
fn is_session_error(status: StatusCode, message: &str) -> bool {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return true;
    }
    let m = message.to_lowercase();
    [
        "sesión",
        "sesion",
        "session",
        "caducad",
        "expired",
        "vuelve a entrar",
    ]
    .iter()
    .any(|needle| m.contains(needle))
}

/// Начало тела ответа для сообщений об ошибках.
fn snippet(body: &str) -> String {
    crate::util::truncate(body.trim(), 200)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_session_errors() {
        assert!(is_session_error(
            StatusCode::BAD_REQUEST,
            "Tu sesión ha caducado. Vuelve a entrar."
        ));
        assert!(is_session_error(StatusCode::UNAUTHORIZED, "whatever"));
        assert!(!is_session_error(StatusCode::NOT_FOUND, "No encontrado."));
        assert!(!is_session_error(
            StatusCode::BAD_REQUEST,
            "Falta el nombre."
        ));
    }
}
