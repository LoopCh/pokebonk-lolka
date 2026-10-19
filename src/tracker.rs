//! Основной цикл: опросить API, сравнить со снимками, отправить события.

use crate::config::Config;
use crate::events;
use crate::lolka::{SendError, Webhook};
use crate::model::Message;
use crate::pokebonk::Api;
use crate::render;
use crate::state::{State, MAX_OUTBOX};
use crate::util;
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Пауза между запросами к API, чтобы не долбить сервер очередью.
const REQUEST_GAP: Duration = Duration::from_millis(300);
/// Пауза между сообщениями в Lolka (лимит — 30 в минуту на вебхук).
const MESSAGE_GAP: Duration = Duration::from_millis(2100);

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("pokebonk-lolka/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .build()
        .context("не удалось создать HTTP-клиент")
}

pub struct Tracker {
    cfg: Config,
    api: Api,
    hook: Webhook,
    state: State,
    last_sweep: Option<Instant>,
    last_season: Option<Instant>,
    last_resolve: Option<Instant>,
    saved_text: String,
}

impl Tracker {
    pub fn new(cfg: Config) -> Result<Self> {
        let mut state = State::load(&cfg.state_path)?;
        if let Some(id) = &cfg.device_id {
            state.device_id.clone_from(id);
        } else if state.device_id.is_empty() {
            state.device_id = util::random_hex32();
        }
        let http = http_client()?;
        let api = Api::new(
            http.clone(),
            cfg.api_base.clone(),
            state.device_id.clone(),
            state.guest_token.clone(),
        );
        let hook = Webhook::new(http, cfg.webhook_url.clone(), cfg.dry_run);
        Ok(Self {
            cfg,
            api,
            hook,
            state,
            last_sweep: None,
            last_season: None,
            last_resolve: None,
            saved_text: String::new(),
        })
    }

    /// Один цикл опроса. Ошибки отдельных запросов логируются и не
    /// останавливают бота.
    pub async fn tick(&mut self) {
        let mut found = Vec::new();
        let mut check = BTreeSet::new();

        if due(self.last_resolve, self.cfg.sweep_every) {
            self.last_resolve = Some(Instant::now());
            self.resolve_names().await;
        }
        if let Some(clan_id) = self.cfg.clan_id.clone() {
            self.poll_clan(&clan_id, &mut found, &mut check).await;
        }

        let tracked = self.tracked_ids();
        self.state.players.retain(|id, _| tracked.contains(id));
        // Новые игроки без снимка — проверить сразу, чтобы запомнить базу.
        check.extend(
            tracked
                .iter()
                .filter(|id| !self.state.players.contains_key(*id))
                .cloned(),
        );
        if due(self.last_sweep, self.cfg.sweep_every) {
            self.last_sweep = Some(Instant::now());
            check.extend(tracked.iter().cloned());
        }
        for id in check.iter().filter(|id| tracked.contains(*id)) {
            self.poll_player(id, &mut found).await;
            tokio::time::sleep(REQUEST_GAP).await;
        }

        if let Some(every) = self.cfg.season_every {
            if due(self.last_season, every) {
                self.last_season = Some(Instant::now());
                self.poll_season(&mut found).await;
            }
        }

        if !found.is_empty() {
            info!("событий: {}", found.len());
            let embeds = found
                .iter()
                .map(|e| render::render(e, &self.cfg.render))
                .collect();
            let messages = render::pack(embeds, self.cfg.webhook_username.as_deref());
            self.queue(messages);
        }
        self.flush().await;

        self.state.guest_token = self.api.token().map(str::to_string);
        self.save();
    }

    /// Кого отслеживаем: участники клана + игроки из POKEBONK_PLAYERS.
    fn tracked_ids(&self) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = self.state.names.values().cloned().collect();
        if let Some(clan) = &self.state.clan {
            ids.extend(clan.members.keys().cloned());
        }
        ids
    }

    /// Ники из настроек -> id. Делается редко: id не меняется при смене ника.
    async fn resolve_names(&mut self) {
        let wanted: Vec<(String, String)> = self
            .cfg
            .players
            .iter()
            .map(|n| (n.to_lowercase(), n.clone()))
            .collect();
        self.state
            .names
            .retain(|key, _| wanted.iter().any(|(k, _)| k == key));

        for (key, name) in wanted {
            if self.state.names.contains_key(&key) {
                continue;
            }
            match self.api.player_by_name(&name).await {
                Ok(p) if !p.id.is_empty() => {
                    info!("игрок «{name}» найден: {}", p.id);
                    if !self.state.players.contains_key(&p.id) {
                        let (_, snap) = events::player_update(None, &p);
                        self.state.players.insert(p.id.clone(), snap);
                    }
                    self.state.names.insert(key, p.id);
                }
                Ok(_) => warn!("игрок «{name}»: в ответе нет id"),
                Err(e) => warn!("игрок «{name}» не найден: {e:#}"),
            }
            tokio::time::sleep(REQUEST_GAP).await;
        }
    }

    async fn poll_clan(
        &mut self,
        clan_id: &str,
        found: &mut Vec<events::Event>,
        check: &mut BTreeSet<String>,
    ) {
        let clan = match self.api.clan(clan_id).await {
            Ok(c) => c,
            Err(e) => {
                warn!("клан: {e:#}");
                return;
            }
        };
        // Пустой состав почти наверняка сбой API, а не распад клана: без этой
        // проверки бот объявил бы, что все вышли.
        if clan.members.is_empty() || clan.id.is_empty() {
            warn!("клан: пустой ответ, пропускаю");
            return;
        }
        let first = self.state.clan.as_ref().map_or(true, |c| c.id != clan.id);
        let diff = events::clan_update(self.state.clan.as_ref(), &clan);
        if first {
            info!(
                "клан {} «{}»: {} участников, запоминаю их забеги",
                events::clan_label(&clan.tag, &clan.name),
                clan.name,
                clan.members.len()
            );
        }
        found.extend(diff.events);
        check.extend(diff.check);
        self.state.clan = Some(diff.snap);
    }

    async fn poll_player(&mut self, id: &str, found: &mut Vec<events::Event>) {
        match self.api.player_by_id(id).await {
            Ok(p) => {
                let prev = self.state.players.get(id);
                if prev.is_none() {
                    debug!("игрок {} ({id}): запоминаю базовое состояние", p.name);
                }
                let (evs, snap) = events::player_update(prev, &p);
                found.extend(evs);
                self.state.players.insert(id.to_string(), snap);
            }
            Err(e) => warn!("игрок {id}: {e:#}"),
        }
    }

    async fn poll_season(&mut self, found: &mut Vec<events::Event>) {
        let info = match self.api.leaderboard().await {
            Ok(lb) => lb.info,
            Err(e) => {
                warn!("сезон: {e:#}");
                return;
            }
        };
        let Some(info) = info else {
            debug!("сезон: в ответе нет info");
            return;
        };
        let remind_ms = i64::try_from(self.cfg.remind_before.as_millis()).unwrap_or(i64::MAX);
        let (evs, snap) = events::season_update(
            &self.state.season,
            &info,
            util::now_ms(),
            remind_ms,
            self.state.clan.as_ref(),
        );
        found.extend(evs);
        self.state.season = snap;
    }

    fn queue(&mut self, messages: Vec<Message>) {
        self.state.outbox.extend(messages);
        let extra = self.state.outbox.len().saturating_sub(MAX_OUTBOX);
        if extra > 0 {
            warn!("очередь сообщений переполнена, выбрасываю {extra} самых старых");
            self.state.outbox.drain(..extra);
        }
    }

    /// Отправляет накопленные сообщения по порядку. При временной ошибке
    /// останавливается и оставляет остаток на следующий цикл.
    async fn flush(&mut self) {
        let mut sent = 0;
        while let Some(msg) = self.state.outbox.first() {
            if sent > 0 {
                tokio::time::sleep(MESSAGE_GAP).await;
            }
            match self.hook.send(msg).await {
                Ok(()) => {
                    self.state.outbox.remove(0);
                    sent += 1;
                }
                Err(SendError::Rejected(e)) => {
                    error!("lolka: {e}; сообщение выброшено");
                    self.state.outbox.remove(0);
                }
                Err(e @ SendError::Temporary(_)) => {
                    warn!(
                        "lolka: {e}; повторю позже (в очереди {})",
                        self.state.outbox.len()
                    );
                    break;
                }
            }
        }
    }

    /// Сохраняет состояние, если оно изменилось.
    pub fn save(&mut self) {
        let text = match self.state.to_text() {
            Ok(t) => t,
            Err(e) => {
                error!("состояние не сериализуется: {e:#}");
                return;
            }
        };
        if text == self.saved_text {
            return;
        }
        match State::save_text(&self.cfg.state_path, &text) {
            Ok(()) => self.saved_text = text,
            Err(e) => error!("состояние не сохранено: {e:#}"),
        }
    }
}

fn due(last: Option<Instant>, every: Duration) -> bool {
    last.map_or(true, |t| t.elapsed() >= every)
}
