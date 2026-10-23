//! Отправка сообщений во входящий вебхук Lolka.
//!
//! Лимиты: 30 запросов в минуту на вебхук. На 429 сервер присылает
//! `Retry-After` (секунды) — ждём столько, сколько просят.

use crate::config::Secret;
use crate::model::Message;
use reqwest::{Client, StatusCode};
use std::fmt;
use std::time::Duration;

const ATTEMPTS: u32 = 4;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub enum SendError {
    /// Сеть, 5xx, 429 — попробуем снова в следующем цикле.
    Temporary(String),
    /// 4xx: Lolka отвергла сообщение; повторять бессмысленно.
    Rejected(String),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporary(m) => write!(f, "временная ошибка: {m}"),
            Self::Rejected(m) => write!(f, "сообщение отклонено: {m}"),
        }
    }
}

pub struct Webhook {
    http: Client,
    url: Secret,
    dry_run: bool,
}

impl Webhook {
    pub fn new(http: Client, url: Secret, dry_run: bool) -> Self {
        Self { http, url, dry_run }
    }

    pub async fn send(&self, msg: &Message) -> Result<(), SendError> {
        if self.dry_run {
            let json = serde_json::to_string_pretty(msg).unwrap_or_default();
            info!("[DRY_RUN] сообщение в Lolka:\n{json}");
            return Ok(());
        }

        let mut last = String::new();
        for attempt in 1..=ATTEMPTS {
            // URL содержит токен, поэтому в ошибках reqwest его вырезаем.
            let result = self
                .http
                .post(self.url.expose())
                .query(&[("wait", "false")])
                .json(msg)
                .send()
                .await;
            let resp = match result {
                Ok(r) => r,
                Err(e) => {
                    last = format!("{:#}", anyhow::Error::new(e.without_url()));
                    warn!("lolka: попытка {attempt}/{ATTEMPTS} не удалась: {last}");
                    tokio::time::sleep(backoff(attempt)).await;
                    continue;
                }
            };

            let status = resp.status();
            if status.is_success() {
                return Ok(());
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let wait = retry_after(
                    resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok()),
                )
                .unwrap_or_else(|| backoff(attempt));
                warn!("lolka: лимит запросов, жду {} с", wait.as_secs());
                tokio::time::sleep(wait).await;
                last = "429 Too Many Requests".to_string();
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            let detail = format!(
                "HTTP {}: {}",
                status.as_u16(),
                crate::util::truncate(&body, 300)
            );
            if status.is_server_error() {
                warn!("lolka: попытка {attempt}/{ATTEMPTS}: {detail}");
                last = detail;
                tokio::time::sleep(backoff(attempt)).await;
                continue;
            }
            return Err(SendError::Rejected(detail));
        }
        Err(SendError::Temporary(last))
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(2u64.saturating_pow(attempt.min(6)))
}

/// `Retry-After` в секундах (может быть дробным).
fn retry_after(header: Option<&str>) -> Option<Duration> {
    let secs: f64 = header?.trim().parse().ok()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    Some(
        Duration::from_secs_f64(secs.min(MAX_RETRY_AFTER.as_secs_f64()))
            + Duration::from_millis(250),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_parsing() {
        assert_eq!(retry_after(Some("2")), Some(Duration::from_millis(2250)));
        assert_eq!(retry_after(Some(" 0.5 ")), Some(Duration::from_millis(750)));
        assert_eq!(
            retry_after(Some("9999")),
            Some(Duration::from_millis(120_250))
        );
        assert_eq!(retry_after(Some("soon")), None);
        assert_eq!(retry_after(None), None);
        assert_eq!(backoff(1), Duration::from_secs(2));
        assert_eq!(backoff(3), Duration::from_secs(8));
    }
}
