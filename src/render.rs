//! Превращает события в карточки (embeds) для Lolka и упаковывает их в сообщения.

use crate::events::{Achievement, Event, PlayerCtx};
use crate::model::{Embed, Field, Footer, Image, Message, Run};
use crate::{names, util};

const GREEN: u32 = 0x0057_F287;
const GOLD: u32 = 0x00F1_C40F;
const GREY: u32 = 0x0095_A5A6;
const BLUE: u32 = 0x0034_98DB;
const ORANGE: u32 = 0x00E6_7E22;
const PURPLE: u32 = 0x009B_59B6;

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
        } => run_embed(player, run, achievements, cfg),
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
                    "⏰ Сезон «{name}» закончится через {}",
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
            format!("🌱 Начался сезон «{name}»"),
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
            simple(GOLD, format!("🏁 Сезон «{name}» завершён"), description)
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

fn run_embed(p: &PlayerCtx, r: &Run, achievements: &[Achievement], cfg: &RenderCfg) -> Embed {
    let title = if r.victory {
        format!("🏆 {} — победа!", p.name)
    } else {
        format!("🏁 {} — забег окончен", p.name)
    };

    let start = names::pokemon(r.species);
    let evolved = r.final_species > 0 && r.final_species != r.species;
    let mons = if evolved {
        format!("**{start}** → **{}**", names::pokemon(r.final_species))
    } else {
        format!("**{start}**")
    };
    let description = format!(
        "{mons}\n{} · тир {} · стадия {}",
        names::biome(&r.biome),
        r.tier,
        r.stage
    );

    let mut fields = vec![
        field("⏱ Время", util::fmt_duration(r.time), true),
        field("💀 Убийства", util::thousands(r.kills), true),
        field(
            "🎯 Режим",
            if r.ranked {
                "рейтинговый"
            } else {
                "обычный"
            }
            .to_string(),
            true,
        ),
    ];
    if !achievements.is_empty() {
        let lines: Vec<String> = achievements.iter().map(achievement_text).collect();
        fields.push(field("Достижения", lines.join("\n"), false));
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
    fn season_cards() {
        let e = render(
            &Event::SeasonFinished {
                name: "GEN 1".into(),
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
                name: "GEN 1".into(),
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
