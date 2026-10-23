//! Живое табло клана: одно сообщение, которое бот редактирует по расписанию.

use crate::events::{clan_label, season_label};
use crate::model::{ClanSnap, DayStats, Embed, Field, Footer, PlayerSnap, SeasonSnap};
use crate::util;
use std::collections::BTreeMap;

/// Сколько участников показывать (в клане может быть до 200).
const SHOW_MEMBERS: usize = 40;
const MIN_MS: i64 = 60_000;

pub fn render(
    clan: &ClanSnap,
    players: &BTreeMap<String, PlayerSnap>,
    season: &SeasonSnap,
    day: &DayStats,
    now_ms: i64,
) -> Embed {
    let mut lines = Vec::new();

    if let Some((n, s)) = clan.seasons.iter().next_back() {
        let name = if season.n == *n {
            season_label(season.n, &season.name)
        } else {
            season_label(*n, "")
        };
        let place = match (s.rank, s.clans) {
            (Some(r), Some(total)) => format!(
                " · **{}** место из {}",
                util::thousands(r),
                util::thousands(total)
            ),
            (Some(r), None) => format!(" · **{}** место", util::thousands(r)),
            _ => String::new(),
        };
        lines.push(format!(
            "🏆 Сезон {name}{place} · **{}** очков",
            util::thousands(s.points)
        ));
    }
    if season.end > now_ms {
        lines.push(format!(
            "⏳ До конца сезона: {}",
            util::fmt_until(season.end - now_ms)
        ));
    }
    lines.push(String::new());

    let mut members: Vec<(&String, &crate::model::MemberSnap)> = clan.members.iter().collect();
    members.sort_by(|a, b| {
        b.1.points
            .cmp(&a.1.points)
            .then_with(|| a.1.name.cmp(&b.1.name))
    });
    for (i, (id, m)) in members.iter().take(SHOW_MEMBERS).enumerate() {
        let role = match m.role.as_str() {
            "leader" => " 👑",
            "officer" | "coleader" | "co-leader" => " ⭐",
            _ => "",
        };
        let mut line = format!(
            "`{:>2}.` **{}**{role} — {}",
            i + 1,
            m.name,
            util::thousands(m.points)
        );
        if let Some(today) = day.players.get(*id).filter(|p| p.runs > 0) {
            line.push_str(&format!(" · сегодня {}", today.runs));
        }
        if let Some(seen) = players.get(*id).map(|p| p.last_run_date).filter(|d| *d > 0) {
            line.push_str(&format!(" · {}", activity(now_ms - seen)));
        }
        lines.push(line);
    }
    if members.len() > SHOW_MEMBERS {
        lines.push(format!("…и ещё {}", members.len() - SHOW_MEMBERS));
    }

    let today_runs: i64 = day.players.values().map(|p| p.runs).sum();
    let today_wins: i64 = day.players.values().map(|p| p.wins).sum();
    let fields = vec![
        Field {
            name: "🎮 Забегов сегодня".to_string(),
            value: util::thousands(today_runs),
            inline: true,
        },
        Field {
            name: "🏆 Побед".to_string(),
            value: util::thousands(today_wins),
            inline: true,
        },
        Field {
            name: "👥 Участников".to_string(),
            value: util::thousands(i64::try_from(members.len()).unwrap_or(i64::MAX)),
            inline: true,
        },
    ];

    Embed {
        title: Some(util::truncate(
            &format!(
                "📋 Табло клана {} {}",
                clan_label(&clan.tag, &clan.name),
                clan.name
            ),
            256,
        )),
        description: Some(util::truncate(&lines.join("\n"), 4000)),
        color: Some(0x00F1_C40F),
        footer: Some(Footer {
            text: "Обновляется автоматически · Pokebonk".to_string(),
        }),
        timestamp: Some(util::iso8601(now_ms)),
        fields,
        ..Embed::default()
    }
}

/// Когда был последний забег: «🟢 только что», «2 ч назад», «3 д назад».
fn activity(ago_ms: i64) -> String {
    let mins = ago_ms.max(0) / MIN_MS;
    match mins {
        0..=29 => "🟢 играет".to_string(),
        30..=59 => format!("{mins} мин назад"),
        60..=1439 => format!("{} ч назад", mins / 60),
        _ => format!("{} д назад", mins / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClanSeasonSnap, DayPlayer, MemberSnap};

    #[test]
    fn board_card() {
        let now = 1_000 * 86_400_000;
        let mut clan = ClanSnap {
            id: "c1".into(),
            name: "Anapa2045".into(),
            tag: "ANP".into(),
            ..ClanSnap::default()
        };
        for (id, name, role, points) in [
            ("a", "homerunwithtati", "leader", 72_750),
            ("b", "Timesan", "member", 29_609),
            ("c", "3x1ee", "member", 0),
        ] {
            clan.members.insert(
                id.into(),
                MemberSnap {
                    name: name.into(),
                    role: role.into(),
                    runs: 0,
                    points,
                },
            );
        }
        clan.seasons.insert(
            2,
            ClanSeasonSnap {
                points: 102_359,
                rank: Some(773),
                clans: Some(1241),
            },
        );
        let mut players = BTreeMap::new();
        players.insert(
            "a".to_string(),
            PlayerSnap {
                last_run_date: now - 5 * MIN_MS,
                ..PlayerSnap::default()
            },
        );
        players.insert(
            "b".to_string(),
            PlayerSnap {
                last_run_date: now - 3 * 60 * MIN_MS,
                ..PlayerSnap::default()
            },
        );
        let season = SeasonSnap {
            n: 2,
            name: String::new(),
            end: now + (12 * 24 + 6) * 60 * MIN_MS,
            ..SeasonSnap::default()
        };
        let mut day = DayStats::default();
        day.players.insert(
            "a".into(),
            DayPlayer {
                runs: 3,
                wins: 1,
                ..DayPlayer::default()
            },
        );

        let e = render(&clan, &players, &season, &day, now);
        assert_eq!(e.title.as_deref(), Some("📋 Табло клана [ANP] Anapa2045"));
        assert_eq!(
            e.description.as_deref(),
            Some(
                "🏆 Сезон №2 · **773** место из 1\u{a0}241 · **102\u{a0}359** очков\n\
                 ⏳ До конца сезона: 12 д 6 ч\n\
                 \n\
                 ` 1.` **homerunwithtati** 👑 — 72\u{a0}750 · сегодня 3 · 🟢 играет\n\
                 ` 2.` **Timesan** — 29\u{a0}609 · 3 ч назад\n\
                 ` 3.` **3x1ee** — 0"
            )
        );
        assert_eq!(e.fields[0].value, "3");
        assert_eq!(e.fields[1].value, "1");
        assert_eq!(e.fields[2].value, "3");
    }

    #[test]
    fn activity_words() {
        assert_eq!(activity(0), "🟢 играет");
        assert_eq!(activity(45 * MIN_MS), "45 мин назад");
        assert_eq!(activity(5 * 60 * MIN_MS), "5 ч назад");
        assert_eq!(activity(3 * 1440 * MIN_MS), "3 д назад");
    }
}
