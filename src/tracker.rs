//! Основной цикл: опросить API, сравнить со снимками, отправить события.

use crate::config::Config;
use crate::lolka::{SendError, Webhook};
use crate::model::Message;
use crate::pokebonk::Api;
use crate::state::{Channel, Outgoing, State, MAX_OUTBOX};
use crate::{board, digest, events, news, render, util};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Пауза между запросами к API, чтобы не долбить сервер очередью.
const REQUEST_GAP: Duration = Duration::from_millis(300);
/// Пауза между сообщениями в Lolka (лимит — 30 в минуту на вебхук).
const MESSAGE_GAP: Duration = Duration::from_millis(2100);
/// Если бот был выключен в момент итогов, опоздавшие итоги публикуются,
/// только пока не прошло столько времени.
const DIGEST_GRACE_MS: i64 = 6 * 3_600_000;

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("pokebonk-lolka/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        // Без пула: запросы идут раз в минуту, а простаивающие соединения кто-то
        // по дороге молча обрывает, и следующий запрос висит до тайм-аута.
        .pool_max_idle_per_host(0)
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .context("не удалось создать HTTP-клиент")
}

pub struct Tracker {
    cfg: Config,
    api: Api,
    hook: Webhook,
    news_hook: Option<Webhook>,
    board_hook: Option<Webhook>,
    digest_hook: Option<Webhook>,
    state: State,
    last_sweep: Option<Instant>,
    last_season: Option<Instant>,
    last_resolve: Option<Instant>,
    last_news: Option<Instant>,
    last_board: Option<Instant>,
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
        let make =
            |url: &crate::config::Secret| Webhook::new(http.clone(), url.clone(), cfg.dry_run);
        let hook = make(&cfg.webhook_url);
        let news_hook = cfg.news_webhook.as_ref().map(make);
        let board_hook = cfg.board_webhook.as_ref().map(make);
        let digest_hook = cfg
            .digest
            .as_ref()
            .and_then(|d| d.webhook.as_ref())
            .map(make);

        // Очередь из версий до 0.3 — в основной канал.
        let old: Vec<Message> = std::mem::take(&mut state.outbox);
        state.queue.extend(old.into_iter().map(|message| Outgoing {
            channel: Channel::Runs,
            message,
        }));

        Ok(Self {
            cfg,
            api,
            hook,
            news_hook,
            board_hook,
            digest_hook,
            state,
            last_sweep: None,
            last_season: None,
            last_resolve: None,
            last_news: None,
            last_board: None,
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

        self.attach_builds(&mut found).await;
        for ev in &found {
            digest::record(&mut self.state.day, ev);
        }

        if !found.is_empty() {
            info!("событий: {}", found.len());
            let embeds = found
                .iter()
                .map(|e| render::render(e, &self.cfg.render))
                .collect();
            let messages = render::pack(embeds, self.cfg.webhook_username.as_deref());
            self.queue(Channel::Runs, messages);
        }

        self.maybe_digest();
        if self.news_hook.is_some() && due(self.last_news, self.cfg.news_every) {
            self.last_news = Some(Instant::now());
            self.poll_news().await;
        }
        self.flush().await;
        if self.board_hook.is_some() && due(self.last_board, self.cfg.board_every) {
            self.last_board = Some(Instant::now());
            self.update_board().await;
        }

        self.state.guest_token = self.api.token().map(str::to_string);
        self.save();
    }

    /// Итоги дня: по расписанию собрать карточку и начать новый период.
    fn maybe_digest(&mut self) {
        let Some(cfg) = self.cfg.digest.clone() else {
            return;
        };
        let zone = self.cfg.zone;
        let now = util::now_ms();
        let day = &mut self.state.day;
        if day.next_at == 0 {
            *day = digest::new_day(now, zone, cfg.minute, self.state.clan.as_ref());
            info!(
                "итоги дня: следующие в {}",
                util::iso8601(self.state.day.next_at)
            );
            return;
        }
        if let Some(clan) = &self.state.clan {
            digest::capture_start(day, clan);
        }
        if now < day.next_at {
            return;
        }

        if now - day.next_at <= DIGEST_GRACE_MS {
            match digest::render(day, self.state.clan.as_ref(), zone) {
                Some(embed) => {
                    info!("итоги дня: игроков {}", day.players.len());
                    let messages = render::pack(vec![embed], self.cfg.webhook_username.as_deref());
                    self.queue(Channel::Digest, messages);
                }
                None => info!("итоги дня: никто не играл, пропускаю"),
            }
        } else {
            warn!("итоги дня пропущены: бот был выключен в момент публикации");
        }
        self.state.day = digest::new_day(now, zone, cfg.minute, self.state.clan.as_ref());
    }

    async fn poll_news(&mut self) {
        let items = match self.api.news().await {
            Ok(items) => items,
            Err(e) => {
                warn!("новости: {e:#}");
                return;
            }
        };
        let first = self.state.news_seen.is_none();
        let (fresh, seen) = news::new_items(self.state.news_seen, &items);
        if first {
            info!("новости: запомнил {} уже вышедших", items.len());
        }
        let messages: Vec<Message> = fresh
            .into_iter()
            .map(|item| {
                info!("новость №{}: {}", item.id, item.text.title);
                Message {
                    content: None,
                    username: self.cfg.webhook_username.clone(),
                    embeds: vec![news::render(item)],
                }
            })
            .collect();
        self.queue(Channel::News, messages);
        self.state.news_seen = seen;
    }

    /// Табло: первое сообщение отправляется, дальше только редактируется.
    async fn update_board(&mut self) {
        let (Some(hook), Some(clan)) = (&self.board_hook, &self.state.clan) else {
            return;
        };
        let embed = board::render(
            clan,
            &self.state.players,
            &self.state.season,
            &self.state.day,
            util::now_ms(),
        );
        let msg = Message {
            content: None,
            username: self.cfg.webhook_username.clone(),
            embeds: vec![embed],
        };
        if let Some(id) = self.state.board_message.clone() {
            match hook.edit(&id, &msg).await {
                Ok(()) => return,
                Err(SendError::Missing) => {
                    info!("табло: сообщение {id} удалено, отправляю новое");
                }
                Err(e) => {
                    warn!("табло: {e}");
                    return;
                }
            }
        }
        match hook.send_returning_id(&msg).await {
            Ok(id) => {
                info!("табло: новое сообщение {id}");
                self.state.board_message = Some(id);
            }
            Err(e) => warn!("табло: {e}"),
        }
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

    /// Подгружает сборки для подходящих забегов. Не получилось — карточка
    /// уйдёт без сборки, это не повод терять событие.
    async fn attach_builds(&mut self, found: &mut [events::Event]) {
        let mode = self.cfg.build_details;
        for ev in found.iter_mut() {
            let events::Event::RunFinished { run, build, .. } = ev else {
                continue;
            };
            let Some(run_id) = run.id.as_deref() else {
                continue;
            };
            if !mode.wants(run.victory, run.time, true) {
                continue;
            }
            match self.api.run_build(run_id).await {
                Ok(b) => *build = Some(Box::new(b)),
                Err(e) => warn!("сборка забега {run_id}: {e:#}"),
            }
            tokio::time::sleep(REQUEST_GAP).await;
        }
    }

    fn queue(&mut self, channel: Channel, messages: Vec<Message>) {
        self.state.queue.extend(
            messages
                .into_iter()
                .map(|message| Outgoing { channel, message }),
        );
        let extra = self.state.queue.len().saturating_sub(MAX_OUTBOX);
        if extra > 0 {
            warn!("очередь сообщений переполнена, выбрасываю {extra} самых старых");
            self.state.queue.drain(..extra);
        }
    }

    fn hook_for(&self, channel: Channel) -> Option<&Webhook> {
        match channel {
            Channel::Runs => Some(&self.hook),
            Channel::Digest => self.digest_hook.as_ref().or(Some(&self.hook)),
            Channel::News => self.news_hook.as_ref(),
        }
    }

    /// Отправляет накопленные сообщения по порядку. При временной ошибке
    /// останавливается и оставляет остаток на следующий цикл.
    async fn flush(&mut self) {
        let mut sent = 0;
        while let Some(out) = self.state.queue.first() {
            let Some(hook) = self.hook_for(out.channel) else {
                warn!("канал {:?} не настроен, сообщение выброшено", out.channel);
                self.state.queue.remove(0);
                continue;
            };
            if sent > 0 {
                tokio::time::sleep(MESSAGE_GAP).await;
            }
            match hook.send(&out.message).await {
                Ok(()) => {
                    self.state.queue.remove(0);
                    sent += 1;
                }
                Err(e @ (SendError::Rejected(_) | SendError::Missing)) => {
                    error!("lolka: {e}; сообщение выброшено");
                    self.state.queue.remove(0);
                }
                Err(e @ SendError::Temporary(_)) => {
                    warn!(
                        "lolka: {e}; повторю позже (в очереди {})",
                        self.state.queue.len()
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
