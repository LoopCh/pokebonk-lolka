//! Превращает события в карточки (embeds) для Lolka и упаковывает их в сообщения.

use crate::events::{Achievement, Event, PlayerCtx};
use crate::model::{Build, Embed, Field, Footer, Image, Message, Run, RunBuild};
use crate::{names, util};

const GREEN: u32 = 0x0057_F287;
const GOLD: u32 = 0x00F1_C40F;
const GREY: u32 = 0x0095_A5A6;
const BLUE: u32 = 0x0034_98DB;
const ORANGE: u32 = 0x00E6_7E22;
const PURPLE: u32 = 0x009B_59B6;

/// Сколько предметов перечислять; остальные — «и ещё N».
const SHOW_ITEMS: usize = 8;

/// Лимиты Lolka: 10 embeds на сообщение и 6000 символов текста на все embeds.
const MAX_EMBEDS: usize = 10;
const MAX_TEXT: usize = 5500;

/// Настройки оформления.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderCfg {
    /// Шаблон картинки покемона, `{id}` — номер в покедексе.
    pub sprite_url: Option<String>,
    /// Шаблон ссылки на профиль, `{id}` и `{name}`.
    pub player_url: Option<String>,
}

pub fn render(ev: &Event, cfg: &RenderCfg) -> Embed {
    match ev {
        Event::RunFinished {
            player,
            run,
            achievements,
            build,
        } => run_embed(player, run, achievements, build.as_deref(), cfg),
        Event::Renamed { old, new } => simple(BLUE, format!("✏️ {old} теперь {new}"), None),
        Event::Joined { name, clan } => {
            simple(BLUE, format!("👋 {name} вступил(а) в клан {clan}"), None)
        }
        Event::Left { name, clan } => {
            simple(GREY, format!("🚪 {name} покинул(а) клан {clan}"), None)
        }
        Event::RoleChanged { name, old, new } => simple(
            PURPLE,
            format!("🎖 {name} теперь {}", names::role(new)),
            Some(format!("Было: {}", names::role(old))),
        ),
        Event::SeasonEndingSoon {
            name,
            end_ms,
            now_ms,
        } => Embed {
            timestamp: Some(util::iso8601(*end_ms)),
            ..simple(
                ORANGE,
                format!(
                    "⏰ Сезон {name} закончится через {}",
                    util::fmt_until(end_ms - now_ms)
                ),
                Some("Последний шанс поднять клан в рейтинге!".to_string()),
            )
        },
        Event::SeasonStarted {
            name,
            end_ms,
            now_ms,
        } => simple(
            PURPLE,
            format!("🌱 Начался сезон {name}"),
            end_ms.map(|end| format!("Продлится {}", util::fmt_until(end - now_ms))),
        ),
        Event::SeasonFinished { name, clan } => {
            let description = clan.as_ref().map(|(label, s)| {
                let place = match (s.rank, s.clans) {
                    (Some(r), Some(total)) => {
                        format!(
                            "**{}** место из {}",
                            util::thousands(r),
                            util::thousands(total)
                        )
                    }
                    (Some(r), None) => format!("**{}** место", util::thousands(r)),
                    _ => "вне рейтинга".to_string(),
                };
                format!(
                    "Клан **{label}**: {place} · {} очков",
                    util::thousands(s.points)
                )
            });
            simple(GOLD, format!("🏁 Сезон {name} завершён"), description)
        }
        Event::Announcement { text } => simple(
            BLUE,
            "📣 Объявление Pokebonk".to_string(),
            Some(util::truncate(text, 4000)),
        ),
    }
}

fn simple(color: u32, title: String, description: Option<String>) -> Embed {
    Embed {
        title: Some(util::truncate(&title, 256)),
        description,
        color: Some(color),
        ..Embed::default()
    }
}

fn run_embed(
    p: &PlayerCtx,
    r: &Run,
    achievements: &[Achievement],
    rb: Option<&RunBuild>,
    cfg: &RenderCfg,
) -> Embed {
    let title = if r.victory {
        format!("🏆 {} — победа!", p.name)
    } else {
        format!("🏁 {} — забег окончен", p.name)
    };

    let start = names::pokemon(r.species);
    let evolved = r.final_species > 0 && r.final_species != r.species;
    let mut mons = if evolved {
        format!("**{start}** → **{}**", names::pokemon(r.final_species))
    } else {
        format!("**{start}**")
    };
    if let Some(level) = rb.map(|b| b.level).filter(|l| *l > 0) {
        mons.push_str(&format!(" · ур. {level}"));
    }
    let description = format!(
        "{mons}\n{} · тир {} · стадия {}",
        names::biome(&r.biome),
        r.tier,
        r.stage
    );

    let mut fields = vec![
        field("⏱ Время", util::fmt_duration(r.time), true),
        field("💀 Убийства", util::thousands(r.kills), true),
    ];
    if let Some(damage) = rb.map(|b| b.damage).filter(|d| *d > 0) {
        fields.push(field("💥 Урон", util::compact(damage), true));
    }
    if !r.ranked {
        fields.push(field("🎯 Режим", "обычный".to_string(), true));
    }
    if !achievements.is_empty() {
        let lines: Vec<String> = achievements.iter().map(achievement_text).collect();
        fields.push(field("Достижения", lines.join("\n"), false));
    }
    if let Some(build) = rb.and_then(|b| b.build.as_ref()) {
        fields.extend(build_fields(build));
    }

    let color = if !achievements.is_empty() {
        GOLD
    } else if r.victory {
        GREEN
    } else {
        GREY
    };

    let shown_species = if evolved { r.final_species } else { r.species };
    let thumbnail = cfg
        .sprite_url
        .as_ref()
        .filter(|_| shown_species > 0)
        .map(|tpl| Image {
            url: tpl.replace("{id}", &shown_species.to_string()),
        });
    let url = cfg.player_url.as_ref().map(|tpl| {
        tpl.replace("{id}", &util::url_encode(&p.id))
            .replace("{name}", &util::url_encode(&p.name))
    });

    let mut footer = Vec::new();
    if let Some(tag) = &p.clan_tag {
        footer.push(format!("[{tag}]"));
    }
    match (p.rank, p.players) {
        (Some(rank), Some(total)) => footer.push(format!(
            "#{} из {}",
            util::thousands(rank),
            util::thousands(total)
        )),
        (Some(rank), None) => footer.push(format!("#{}", util::thousands(rank))),
        _ => {}
    }
    footer.push("Pokebonk".to_string());

    Embed {
        title: Some(util::truncate(&title, 256)),
        description: Some(description),
        url,
        color: Some(color),
        timestamp: (r.date > 0).then(|| util::iso8601(r.date)),
        footer: Some(Footer {
            text: footer.join(" · "),
        }),
        thumbnail,
        fields,
    }
}

/// Сборка: приёмы с долей урона, главные предметы, витамины, ключевые статы.
fn build_fields(b: &Build) -> Vec<Field> {
    let mut out = Vec::new();

    let mut moves: Vec<_> = b.moves.iter().filter(|m| !m.k.is_empty()).collect();
    moves.sort_by(|a, b| b.dmg.cmp(&a.dmg).then_with(|| a.k.cmp(&b.k)));
    let total: i64 = moves.iter().map(|m| m.dmg.max(0)).sum();
    if !moves.is_empty() {
        let lines: Vec<String> = moves
            .iter()
            .map(|m| {
                let share = if total > 0 {
                    // Доля в процентах, точность не важна.
                    #[allow(clippy::cast_precision_loss)]
                    let pct = m.dmg.max(0) as f64 * 100.0 / total as f64;
                    format!(" ({pct:.0}%)")
                } else {
                    String::new()
                };
                format!(
                    "**{}** · ур. {} — {}{share}",
                    names::move_name(&m.k),
                    m.lv,
                    util::compact(m.dmg)
                )
            })
            .collect();
        out.push(field("⚔️ Приёмы", lines.join("\n"), false));
    }

    // Сначала то, чего набрано больше всего: это и определяет сборку.
    let mut items: Vec<(&str, i64)> = b
        .items
        .iter()
        .map(|(k, v)| {
            // Количество предметов — небольшое целое.
            #[allow(clippy::cast_possible_truncation)]
            let n = v.unwrap_or(1.0).round() as i64;
            (k.as_str(), n)
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !items.is_empty() {
        let mut parts: Vec<String> = items
            .iter()
            .take(SHOW_ITEMS)
            .map(|(k, n)| {
                if *n > 1 {
                    format!("{} ×{n}", names::item(k))
                } else {
                    names::item(k)
                }
            })
            .collect();
        if items.len() > SHOW_ITEMS {
            parts.push(format!("и ещё {}", items.len() - SHOW_ITEMS));
        }
        let total: i64 = items.iter().map(|(_, n)| n).sum();
        out.push(field(
            &format!("🎒 Предметы · {total}"),
            parts.join(", "),
            false,
        ));
    }

    if !b.vits.is_empty() {
        let parts: Vec<String> = b
            .vits
            .iter()
            .map(|v| format!("{} {}", names::vitamin(&v.k), v.lv))
            .collect();
        out.push(field("💊 Витамины", parts.join(" · "), false));
    }

    let stats = key_stats(b);
    if !stats.is_empty() {
        out.push(field("📊 Ключевые статы", stats, false));
    }
    out
}

/// Несколько характеристик, по которым понятна сборка. Остальные ~30 чисел
/// из игры в чате только мешают.
fn key_stats(b: &Build) -> String {
    let mut lines = Vec::new();
    if !b.passive.is_empty() {
        lines.push(format!("🧬 {}", names::passive(&b.passive)));
    }

    let mut attack = Vec::new();
    if let Some(v) = b.stat("damage") {
        let mut s = format!("Урон {}", util::pct(v));
        if let (Some(sp), Some(ph)) = (b.stat("special"), b.stat("physical")) {
            s.push_str(&format!(
                " (спец. {} · физ. {})",
                util::pct(sp),
                util::pct(ph)
            ));
        }
        attack.push(s);
    }
    if let Some(v) = b.stat("attackSpeed") {
        attack.push(format!("Ск. атаки {}", util::pct(v)));
    }
    if !attack.is_empty() {
        lines.push(format!("⚔️ {}", attack.join(" · ")));
    }

    let mut extra = Vec::new();
    if let Some(v) = b.stat("critChance") {
        let mut s = format!("Крит {}", util::pct(v));
        if let Some(cd) = b.stat("critDamage") {
            s.push_str(&format!(" ×{}", util::pct(cd)));
        }
        extra.push(s);
    }
    if let Some(v) = b.stat("projectiles") {
        extra.push(format!("Снаряды {}", util::num(v)));
    }
    if let Some(v) = b.stat("area") {
        extra.push(format!("Область {}", util::pct(v)));
    }
    if !extra.is_empty() {
        lines.push(format!("🎯 {}", extra.join(" · ")));
    }

    let mut defense = Vec::new();
    if let Some(v) = b.stat("maxHp") {
        defense.push(format!("ОЗ {}", util::num(v)));
    }
    if let Some(v) = b.stat("armor") {
        defense.push(format!("Броня {}", util::num(v)));
    }
    if let Some(v) = b.stat("regen").filter(|v| *v > 0.0) {
        defense.push(format!("Реген {}/с", util::num(v)));
    }
    if let Some(v) = b.stat("lifesteal").filter(|v| *v > 0.0) {
        defense.push(format!("Вампиризм {}", util::pct(v)));
    }
    if let Some(v) = b.stat("shieldMax").filter(|v| *v > 0.0) {
        defense.push(format!("Щит {}", util::num(v)));
    }
    if !defense.is_empty() {
        lines.push(format!("🛡 {}", defense.join(" · ")));
    }

    let mut misc = Vec::new();
    if let Some(v) = b.stat("luck") {
        misc.push(format!("Удача {}", util::num(v)));
    }
    if let Some(v) = b.stat("xpGain") {
        misc.push(format!("Опыт {}", util::pct(v)));
    }
    if !misc.is_empty() {
        lines.push(format!("🍀 {}", misc.join(" · ")));
    }
    lines.join("\n")
}

fn field(name: &str, value: String, inline: bool) -> Field {
    Field {
        name: name.to_string(),
        value,
        inline,
    }
}

pub fn achievement_text(a: &Achievement) -> String {
    match a {
        Achievement::FirstWin => "🥇 Первая победа!".to_string(),
        Achievement::Shinies(n) => format!("✨ Шайни: +{n}"),
        Achievement::Legendaries(n) => format!("👑 Легендарки: +{n}"),
        Achievement::TimeMedals(n) => format!("🏅 Медали за время: +{n}"),
        Achievement::Caught(n) => format!("📘 Новые покемоны в коллекции: +{n}"),
        Achievement::MaxLevel(l) => format!("📈 Рекорд уровня: {l}"),
        Achievement::MaxTime(t) => format!("⏳ Рекорд по времени: {}", util::fmt_duration(*t)),
    }
}

/// Сколько символов embed засчитывает в лимит Lolka.
fn text_len(e: &Embed) -> usize {
    let opt = |s: &Option<String>| s.as_ref().map_or(0, |s| s.chars().count());
    opt(&e.title)
        + opt(&e.description)
        + e.footer.as_ref().map_or(0, |f| f.text.chars().count())
        + e.fields
            .iter()
            .map(|f| f.name.chars().count() + f.value.chars().count())
            .sum::<usize>()
}

/// Складывает карточки в сообщения с учётом лимитов Lolka.
pub fn pack(embeds: Vec<Embed>, username: Option<&str>) -> Vec<Message> {
    let make = |embeds: Vec<Embed>| Message {
        content: None,
        username: username.map(str::to_string),
        embeds,
    };
    let mut out = Vec::new();
    let mut current: Vec<Embed> = Vec::new();
    let mut len = 0;
    for e in embeds {
        let l = text_len(&e);
        if !current.is_empty() && (current.len() == MAX_EMBEDS || len + l > MAX_TEXT) {
            out.push(make(std::mem::take(&mut current)));
            len = 0;
        }
        len += l;
        current.push(e);
    }
    if !current.is_empty() {
        out.push(make(current));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ClanSeasonSnap;

    fn ctx() -> PlayerCtx {
        PlayerCtx {
            id: "p1".into(),
            name: "3x1ee".into(),
            clan_tag: Some("ANP".into()),
            rank: Some(41_800),
            players: Some(122_096),
        }
    }

    fn cfg() -> RenderCfg {
        RenderCfg {
            sprite_url: Some("https://img.example/{id}.png".into()),
            player_url: Some("https://pokebonk.com/?p={name}".into()),
        }
    }

    #[test]
    fn winning_run_card() {
        let run = Run {
            date: 1_792_318_715_661,
            species: 1,
            final_species: 3,
            biome: "forest".into(),
            stage: 4,
            time: 1947.677,
            kills: 28_077,
            victory: true,
            ranked: true,
            ..Run::default()
        };
        let e = render(
            &Event::RunFinished {
                player: ctx(),
                run,
                achievements: vec![],
                build: None,
            },
            &cfg(),
        );
        assert_eq!(e.title.as_deref(), Some("🏆 3x1ee — победа!"));
        assert_eq!(
            e.description.as_deref(),
            Some("**Бульбазавр** → **Венузавр**\nЛес · тир 0 · стадия 4")
        );
        assert_eq!(e.color, Some(GREEN));
        assert_eq!(e.fields[0].value, "32:28");
        assert_eq!(e.fields[1].value, "28\u{a0}077");
        assert_eq!(e.thumbnail.unwrap().url, "https://img.example/3.png");
        assert_eq!(e.url.as_deref(), Some("https://pokebonk.com/?p=3x1ee"));
        assert_eq!(
            e.footer.unwrap().text,
            "[ANP] · #41\u{a0}800 из 122\u{a0}096 · Pokebonk"
        );
        assert_eq!(e.timestamp.as_deref(), Some("2026-10-18T10:18:35.661Z"));
    }

    #[test]
    fn losing_run_with_achievements_is_gold() {
        let run = Run {
            species: 92,
            final_species: 92,
            biome: "desert".into(),
            ..Run::default()
        };
        let e = render(
            &Event::RunFinished {
                player: ctx(),
                run,
                achievements: vec![Achievement::Shinies(1)],
                build: None,
            },
            &RenderCfg::default(),
        );
        assert_eq!(e.title.as_deref(), Some("🏁 3x1ee — забег окончен"));
        assert!(e.description.unwrap().starts_with("**Гастли**\nПустыня"));
        assert_eq!(e.color, Some(GOLD));
        assert_eq!(e.fields[3].value, "✨ Шайни: +1");
        assert!(e.thumbnail.is_none());
        assert!(e.timestamp.is_none());
    }

    #[test]
    fn run_card_with_build() {
        let rb: RunBuild = serde_json::from_str(crate::model::tests::BUILD).unwrap();
        let run = Run {
            species: 1,
            final_species: 3,
            biome: "forest".into(),
            tier: 0,
            stage: 4,
            time: 1787.05,
            kills: 26_645,
            victory: true,
            ranked: true,
            ..Run::default()
        };
        let e = render(
            &Event::RunFinished {
                player: ctx(),
                run,
                achievements: vec![],
                build: Some(Box::new(rb)),
            },
            &RenderCfg::default(),
        );
        let text = serde_json::to_string_pretty(&e).unwrap();
        println!("{text}");
        assert!(e
            .description
            .as_deref()
            .unwrap()
            .starts_with("**Бульбазавр** → **Венузавр** · ур. 195\n"));
        let names: Vec<&str> = e.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "⏱ Время",
                "💀 Убийства",
                "💥 Урон",
                "⚔️ Приёмы",
                "🎒 Предметы · 46",
                "💊 Витамины",
                "📊 Ключевые статы"
            ]
        );
        assert_eq!(e.fields[2].value, "238M");
        assert_eq!(
            e.fields[3].value,
            "**Fu T Grass** · ур. 5 — 117M (75%)\n\
             **Solar Beam** · ур. 8 — 35.9M (23%)\n\
             **Razor Leaf** · ур. 8 — 2.8M (2%)"
        );
        assert_eq!(
            e.fields[4].value,
            "Leftovers ×4, Счастливое яйцо ×4, PP Max ×4, Делитель опыта ×3, Soothe Bell ×3, \
             Звёздный осколок ×3, Choice Band ×2, Muscle Band ×2, и ещё 19"
        );
        assert_eq!(e.fields[5].value, "Exp Candy 10 · Calcium 10 · Ether 10");
        assert_eq!(
            e.fields[6].value,
            "🧬 Photosynthesis\n\
             ⚔️ Урон 308% (спец. 341% · физ. 202%) · Ск. атаки 304%\n\
             🎯 Крит 27% ×275% · Снаряды 4 · Область 145%\n\
             🛡 ОЗ 174 · Броня 101 · Реген 4.9/с · Вампиризм 3%\n\
             🍀 Удача 110 · Опыт 1110%"
        );
        assert!(text_len(&e) < 1500, "карточка слишком длинная");
    }

    #[test]
    fn season_cards() {
        let e = render(
            &Event::SeasonFinished {
                name: "«GEN 1»".into(),
                clan: Some((
                    "[ANP]".into(),
                    ClanSeasonSnap {
                        points: 102_359,
                        rank: Some(773),
                        clans: Some(1241),
                    },
                )),
            },
            &RenderCfg::default(),
        );
        assert_eq!(
            e.description.as_deref(),
            Some("Клан **[ANP]**: **773** место из 1\u{a0}241 · 102\u{a0}359 очков")
        );

        let e = render(
            &Event::SeasonEndingSoon {
                name: "«GEN 1»".into(),
                end_ms: 10 * 3_600_000,
                now_ms: 0,
            },
            &RenderCfg::default(),
        );
        assert_eq!(
            e.title.as_deref(),
            Some("⏰ Сезон «GEN 1» закончится через 10 ч")
        );
    }

    #[test]
    fn packing_respects_limits() {
        let small = |i: usize| Embed {
            title: Some(format!("t{i}")),
            ..Embed::default()
        };
        let msgs = pack((0..23).map(small).collect(), Some("bot"));
        assert_eq!(
            msgs.iter().map(|m| m.embeds.len()).collect::<Vec<_>>(),
            vec![10, 10, 3]
        );
        assert_eq!(msgs[0].username.as_deref(), Some("bot"));

        let big = || Embed {
            description: Some("x".repeat(3000)),
            ..Embed::default()
        };
        let msgs = pack(vec![big(), big(), big()], None);
        assert_eq!(msgs.len(), 3);
        assert!(pack(vec![], None).is_empty());
    }
}
