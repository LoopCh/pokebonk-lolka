//! Типы данных: ответы API Pokebonk, снимки состояния и сообщения вебхука Lolka.
//!
//! В ответах API много полей; здесь описаны только нужные боту. Всё помечено
//! `#[serde(default)]`, чтобы новое или пропавшее поле не ломало разбор.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// API Pokebonk
// ---------------------------------------------------------------------------

/// Ответ `GET /players?id=…` или `?name=…`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub id: String,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub name: String,
    pub stats: Stats,
    pub recent: Vec<Run>,
    pub clan: Option<ClanRef>,
    pub standing: Option<Standing>,
}

/// Общая статистика игрока (`stats`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Stats {
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub runs: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub wins: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub kills: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub bosses: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub legendaries: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub shinies: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub caught: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub max_level: i64,
    #[serde(deserialize_with = "crate::serde_util::float")]
    pub max_time: f64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub time_medals: i64,
}

/// Один забег из `recent`. У нерейтинговых забегов нет `id`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Run {
    pub id: Option<String>,
    /// Время окончания, мс Unix.
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub date: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub species: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub final_species: i64,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub biome: String,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub tier: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub stage: i64,
    /// Длительность, секунды.
    #[serde(deserialize_with = "crate::serde_util::float")]
    pub time: f64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub kills: i64,
    pub victory: bool,
    pub ranked: bool,
}

/// Клан в профиле игрока.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClanRef {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub tag: String,
}

/// Место игрока в общем рейтинге.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Standing {
    #[serde(deserialize_with = "crate::serde_util::opt_int")]
    pub rank: Option<i64>,
    #[serde(deserialize_with = "crate::serde_util::opt_int")]
    pub players: Option<i64>,
}

/// Ответ `GET /clans/{id}` (нужна гостевая сессия).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clan {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub id: String,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub name: String,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub tag: String,
    pub members: Vec<Member>,
    pub season: Option<ClanSeason>,
}

/// Участник клана.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Member {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub id: String,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub name: String,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub role: String,
    /// Забеги в зачёт клана (считаются с момента вступления).
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub runs: i64,
}

/// Результат клана в текущем сезоне.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClanSeason {
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub n: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub points: i64,
    #[serde(deserialize_with = "crate::serde_util::opt_int")]
    pub rank: Option<i64>,
    #[serde(deserialize_with = "crate::serde_util::opt_int")]
    pub clans: Option<i64>,
}

/// Ответ `GET /leaderboard?mode=kills`; боту нужен только блок `info` о сезоне.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Leaderboard {
    pub info: Option<SeasonInfo>,
}

/// Информация о сезоне.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeasonInfo {
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub n: i64,
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub name: String,
    /// Конец сезона, мс Unix.
    #[serde(deserialize_with = "crate::serde_util::opt_int")]
    pub end: Option<i64>,
    pub live: bool,
    /// Объявление от разработчиков (обычно пустое).
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub message: String,
}

/// Ответ `POST /auth/guest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuestAuth {
    pub name: String,
    pub token: String,
}

/// Ответ `GET /runs/{id}/build`: подробности одного забега.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunBuild {
    /// Весь нанесённый урон.
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub damage: i64,
    /// Уровень покемона в конце забега.
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub level: i64,
    pub build: Option<Build>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    /// Приёмы с уровнем и уроном.
    pub moves: Vec<BuildMove>,
    /// Предмет -> сколько штук.
    pub items: BTreeMap<String, Option<f64>>,
    /// Витамины с уровнем.
    pub vits: Vec<BuildVitamin>,
    /// Итоговые характеристики: множители (1.0 = 100 %) и абсолютные числа.
    pub stats: BTreeMap<String, Option<f64>>,
    /// Пассивная способность.
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub passive: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildMove {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub k: String,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub lv: i64,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub dmg: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildVitamin {
    #[serde(deserialize_with = "crate::serde_util::string")]
    pub k: String,
    #[serde(deserialize_with = "crate::serde_util::int")]
    pub lv: i64,
}

impl Build {
    /// Значение характеристики, если оно есть.
    pub fn stat(&self, key: &str) -> Option<f64> {
        self.stats
            .get(key)
            .copied()
            .flatten()
            .filter(|v| v.is_finite())
    }
}

/// Ошибка API: `{"error": "Tu sesión ha caducado. Vuelve a entrar."}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

// ---------------------------------------------------------------------------
// Снимки состояния (хранятся в state.json)
// ---------------------------------------------------------------------------

/// Что бот помнит об игроке.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSnap {
    pub name: String,
    /// Дата самого свежего уже обработанного забега.
    pub last_run_date: i64,
    pub stats: Stats,
}

/// Что бот помнит о клане.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClanSnap {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub members: BTreeMap<String, MemberSnap>,
    /// Результаты клана по номерам сезонов (несколько последних).
    pub seasons: BTreeMap<i64, ClanSeasonSnap>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemberSnap {
    pub name: String,
    pub role: String,
    pub runs: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClanSeasonSnap {
    pub points: i64,
    pub rank: Option<i64>,
    pub clans: Option<i64>,
}

/// Что бот помнит о сезоне.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeasonSnap {
    /// 0 — сезон ещё ни разу не загружался.
    pub n: i64,
    pub name: String,
    pub end: i64,
    pub reminded: bool,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Вебхук Lolka (формат как у Discord)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub embeds: Vec<Embed>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Embed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<Footer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Footer {
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Image {
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Field {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // Ответы, сокращённые из реальных, с вымышленными никами.
    const PLAYER: &str = r#"{"id":"p1","name":"ash","guest":false,"stats":{"runs":5,"wins":1,"kills":32394,"bosses":4,"legendaries":1,"shinies":2,"maxLevel":119,"maxTime":1947.6769711983093,"caught":5,"timeMedals":1},"recent":[{"id":"0efc","date":1792332447091,"species":92,"finalSpecies":92,"biome":"desert","tier":0,"stage":1,"time":98.00093,"kills":63,"victory":false,"ranked":true},{"date":1792315002591,"species":4,"finalSpecies":4,"biome":"forest","tier":0,"stage":1,"time":308.5061,"kills":811,"victory":false,"ranked":false}],"standing":{"rank":41800,"players":122096},"clan":{"id":"c1","name":"Team","tag":"ANP","role":"member"},"maps":[{"time":null}]}"#;

    const CLAN: &str = r#"{"id":"c1","name":"Team","tag":"ANP","max":30,"me":null,"members":[{"id":"p0","name":"misty","role":"leader","joined":1792334127528,"points":72750,"runs":4},{"id":"p1","name":"ash","role":"member","points":0,"runs":0}],"season":{"n":1,"points":102359,"rank":773,"clans":1241,"cutoff":17361475},"endless":{"season":{"points":0,"rank":null}}}"#;

    const LEADERBOARD: &str = r#"{"board":"kills:global","entries":[],"me":null,"players":123472,"season":1,"ends":1792540800000,"info":{"n":1,"name":"GEN 1","start":1792081800000,"end":1792540800000,"live":true,"dust":1,"message":"","next":null}}"#;

    #[test]
    fn parses_player() {
        let p: Player = serde_json::from_str(PLAYER).unwrap();
        assert_eq!(p.name, "ash");
        assert_eq!(p.stats.wins, 1);
        assert_eq!(p.stats.max_level, 119);
        assert!((p.stats.max_time - 1947.677).abs() < 0.01);
        assert_eq!(p.recent.len(), 2);
        assert_eq!(p.recent[0].final_species, 92);
        assert_eq!(p.recent[1].id, None);
        assert!(!p.recent[1].ranked);
        assert_eq!(p.clan.unwrap().tag, "ANP");
        assert_eq!(p.standing.unwrap().rank, Some(41800));
    }

    #[test]
    fn parses_clan_and_season() {
        let c: Clan = serde_json::from_str(CLAN).unwrap();
        assert_eq!(c.members.len(), 2);
        assert_eq!(c.members[0].role, "leader");
        assert_eq!(c.season.unwrap().rank, Some(773));

        let lb: Leaderboard = serde_json::from_str(LEADERBOARD).unwrap();
        let info = lb.info.unwrap();
        assert_eq!(info.name, "GEN 1");
        assert_eq!(info.end, Some(1_792_540_800_000));
        assert!(info.live);
    }

    /// Реальный ответ `/runs/{id}/build`, ник заменён.
    pub const BUILD: &str = r#"{"id":"482a03d4b41c4cd8aec8fe26c283b399","user":{"id":"u1","name":"ash"},"date":1792651509156,"species":1,"finalSpecies":3,"biome":"forest","tier":0,"stage":4,"time":1787.0536,"kills":26645,"damage":237565030,"level":195,"victory":true,"ranked":true,"moves":["fu-t-grass","razor-leaf","solar-beam"],"items":["lucky-egg","muscle-band"],"build":{"vits":[{"k":"exp-candy","lv":10},{"k":"calcium","lv":10},{"k":"ether","lv":10}],"items":{"pp-max":4,"big-root":1,"life-orb":1,"exp-share":3,"flame-orb":1,"leftovers":4,"lucky-egg":4,"toxic-orb":1,"zoom-lens":1,"dusk-stone":1,"light-clay":1,"quick-claw":2,"rare-candy":1,"razor-claw":2,"smoke-ball":1,"star-piece":3,"choice-band":2,"cleanse-tag":1,"expert-belt":1,"master-ball":1,"muscle-band":2,"soothe-bell":3,"choice-scarf":1,"choice-specs":1,"quick-powder":1,"sitrus-berry":1,"wise-glasses":1},"moves":[{"k":"fu-t-grass","lv":5,"dmg":116694510},{"k":"razor-leaf","lv":8,"dmg":2829537},{"k":"solar-beam","lv":8,"dmg":35910603}],"stats":{"area":1.45,"gold":1.5,"luck":109.61,"slam":1,"armor":101.2,"maxHp":174.28,"regen":4.91,"damage":3.08,"thorns":0,"xpGain":11.1,"evasion":0.07,"healing":2,"powerup":1,"special":3.41,"duration":1.49,"megaCrit":0,"overheal":0,"physical":2.02,"lifesteal":0.03,"moveSpeed":1.31,"projSpeed":1.15,"shieldMax":0,"critChance":0.27,"critDamage":2.75,"eliteSpawn":1,"extraJumps":0,"jumpHeight":1,"attackSpeed":3.04,"eliteDamage":1.15,"pickupRange":12,"projectiles":4,"statusChance":1.1,"statusDamage":1.1,"superEffective":1.3},"passive":"photosynthesis"}}"#;

    #[test]
    fn parses_build() {
        let b: RunBuild = serde_json::from_str(BUILD).unwrap();
        assert_eq!(b.damage, 237_565_030);
        assert_eq!(b.level, 195);
        let build = b.build.unwrap();
        assert_eq!(build.moves.len(), 3);
        assert_eq!(build.moves[0].dmg, 116_694_510);
        assert_eq!(build.items.len(), 27);
        assert_eq!(build.items["leftovers"], Some(4.0));
        assert_eq!(build.vits[2].k, "ether");
        assert_eq!(build.stat("critChance"), Some(0.27));
        assert_eq!(build.stat("nope"), None);
        assert_eq!(build.passive, "photosynthesis");

        let empty: RunBuild = serde_json::from_str(r#"{"build":null,"damage":null}"#).unwrap();
        assert!(empty.build.is_none());
        assert_eq!(empty.damage, 0);
    }

    #[test]
    fn tolerates_nulls_and_floats() {
        let r: Run = serde_json::from_str(
            r#"{"date":1.7e12,"species":null,"time":null,"kills":10.0,"biome":null}"#,
        )
        .unwrap();
        assert_eq!(r.date, 1_700_000_000_000);
        assert_eq!(r.species, 0);
        assert_eq!(r.kills, 10);
        assert_eq!(r.biome, "");
    }

    #[test]
    fn error_body_is_not_a_clan() {
        let body = r#"{"error":"Tu sesión ha caducado. Vuelve a entrar."}"#;
        assert!(serde_json::from_str::<ApiError>(body).is_ok());
        assert!(serde_json::from_str::<ApiError>(CLAN).is_err());
    }

    #[test]
    fn message_skips_empty_fields() {
        let m = Message {
            embeds: vec![Embed {
                title: Some("t".into()),
                ..Embed::default()
            }],
            ..Message::default()
        };
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"embeds":[{"title":"t"}]}"#
        );
    }
}
