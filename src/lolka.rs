//! Отправка и редактирование сообщений через входящий вебхук Lolka.
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
    /// Сообщения больше нет (его удалили) — для редактирования табло.
    Missing,
    /// 4xx: Lolka отвергла сообщение; повторять бессмысленно.
    Rejected(String),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporary(m) => write!(f, "временная ошибка: {m}"),
            Self::Missing => write!(f, "сообщение не найдено"),
            Self::Rejected(m) => write!(f, "сообщение отклонено: {m}"),
        }
    }
}

#[derive(Clone, Copy)]
enum Op<'a> {
    /// Новое сообщение; `wait` — вернуть созданное сообщение (нужен его id).
    Post { wait: bool },
    /// Изменить своё сообщение.
    Edit { id: &'a str },
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

    /// Отправить сообщение.
    pub async fn send(&self, msg: &Message) -> Result<(), SendError> {
        self.execute(Op::Post { wait: false }, msg)
            .await
            .map(|_| ())
    }

    /// Отправить сообщение и вернуть его id (чтобы потом редактировать).
    pub async fn send_returning_id(&self, msg: &Message) -> Result<String, SendError> {
        let body = self.execute(Op::Post { wait: true }, msg).await?;
        message_id(&body).ok_or_else(|| {
            SendError::Rejected(format!(
                "в ответе нет id сообщения: {}",
                crate::util::truncate(&body, 200)
            ))
        })
    }

    /// Заменить содержимое ранее отправленного сообщения.
    pub async fn edit(&self, id: &str, msg: &Message) -> Result<(), SendError> {
        self.execute(Op::Edit { id }, msg).await.map(|_| ())
    }

    /// Запрос с повторами; возвращает тело ответа.
    async fn execute(&self, op: Op<'_>, msg: &Message) -> Result<String, SendError> {
        if self.dry_run {
            let json = serde_json::to_string_pretty(msg).unwrap_or_default();
            let what = match op {
                Op::Post { .. } => "сообщение".to_string(),
                Op::Edit { id } => format!("правка сообщения {id}"),
            };
            info!("[DRY_RUN] {what} в Lolka:\n{json}");
            return Ok(r#"{"id":"dry-run"}"#.to_string());
        }

        let mut last = String::new();
        for attempt in 1..=ATTEMPTS {
            // URL содержит токен, поэтому в ошибках reqwest его вырезаем.
            let request = match op {
                Op::Post { wait } => self
                    .http
                    .post(self.url.expose())
                    .query(&[("wait", if wait { "true" } else { "false" })]),
                Op::Edit { id } => self
                    .http
                    .patch(format!("{}/messages/{id}", self.url.expose())),
            };
            let resp = match request.json(msg).send().await {
                Ok(r) => r,
                Err(e) => {
                    last = format!("{:#}", anyhow::Error::new(e.without_url()));
                    warn!("lolka: попытка {attempt}/{ATTEMPTS} не удалась: {last}");
                    tokio::time::sleep(backoff(attempt)).await;
                    continue;
                }
            };

            let status = resp.status();
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
            if status.is_success() {
                return Ok(body);
            }
            if status == StatusCode::NOT_FOUND && matches!(op, Op::Edit { .. }) {
                return Err(SendError::Missing);
            }
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

/// id из ответа Lolka: число (snowflake) или строка.
fn message_id(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    match v.get("id")? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
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

    #[test]
    fn message_ids() {
        // Ответ Lolka из переписки: id — большое число.
        assert_eq!(
            message_id(r#"{"id":928823227401216,"content":"x"}"#).as_deref(),
            Some("928823227401216")
        );
        assert_eq!(message_id(r#"{"id":"42"}"#).as_deref(), Some("42"));
        assert_eq!(message_id(r#"{"content":"x"}"#), None);
        assert_eq!(message_id("not json"), None);
    }
}
