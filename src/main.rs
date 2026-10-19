//! pokebonk-lolka — сообщает в канал Lolka о забегах игроков Pokebonk.

#[macro_use]
mod log;

mod config;
mod events;
mod lolka;
mod model;
mod names;
mod pokebonk;
mod render;
mod serde_util;
mod state;
mod tracker;
mod util;

use anyhow::{bail, Result};
use config::Config;
use model::{Embed, Footer, Message};
use std::process::ExitCode;

const HELP: &str = "\
pokebonk-lolka — уведомления о забегах Pokebonk в канал Lolka

Использование:
  pokebonk-lolka                 запустить бота
  pokebonk-lolka test-webhook    отправить тестовое сообщение и выйти
  pokebonk-lolka --version       показать версию

Настройки задаются переменными окружения, см. bot.env.example.";

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("-h" | "--help" | "help") => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Some("-V" | "--version" | "version") => {
            println!("pokebonk-lolka {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Ошибка настроек: {e}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    log::init(&cfg.log_level);

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("не удалось запустить tokio: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async move {
        match command.as_deref() {
            None | Some("run") => run(cfg).await,
            Some("test-webhook") => test_webhook(cfg).await,
            Some(other) => bail!("неизвестная команда «{other}», см. --help"),
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cfg: Config) -> Result<()> {
    info!(
        "pokebonk-lolka {} запущен: клан {}, игроки {:?}, опрос клана раз в {} с, всех игроков раз в {} с{}",
        env!("CARGO_PKG_VERSION"),
        cfg.clan_id.as_deref().unwrap_or("—"),
        cfg.players,
        cfg.clan_every.as_secs(),
        cfg.sweep_every.as_secs(),
        if cfg.dry_run { " [DRY_RUN]" } else { "" },
    );
    let every = cfg.clan_every;
    let mut tracker = tracker::Tracker::new(cfg)?;

    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = ticker.tick() => tracker.tick().await,
            () = &mut shutdown => {
                info!("получен сигнал остановки, выхожу");
                break;
            },
        }
    }
    tracker.save();
    Ok(())
}

async fn test_webhook(cfg: Config) -> Result<()> {
    let hook = lolka::Webhook::new(
        tracker::http_client()?,
        cfg.webhook_url.clone(),
        cfg.dry_run,
    );
    let msg = Message {
        content: None,
        username: cfg.webhook_username.clone(),
        embeds: vec![Embed {
            title: Some("✅ pokebonk-lolka на связи".to_string()),
            description: Some(format!(
                "Клан: {}\nИгроки: {}",
                cfg.clan_id.as_deref().unwrap_or("—"),
                if cfg.players.is_empty() {
                    "—".to_string()
                } else {
                    cfg.players.join(", ")
                }
            )),
            color: Some(0x0057_F287),
            timestamp: Some(util::iso8601(util::now_ms())),
            footer: Some(Footer {
                text: format!("версия {}", env!("CARGO_PKG_VERSION")),
            }),
            ..Embed::default()
        }],
    };
    hook.send(&msg).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    info!("тестовое сообщение отправлено");
    Ok(())
}

/// Ctrl+C или SIGTERM (его шлёт `docker stop`).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {},
                    _ = term.recv() => {},
                }
            }
            Err(e) => {
                warn!("SIGTERM не перехватывается ({e}), жду только Ctrl+C");
                tokio::signal::ctrl_c().await.ok();
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
    }
}
