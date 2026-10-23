//! Итоги дня: копим статистику забегов и раз в сутки публикуем сводку.

use crate::events::{clan_label, Achievement, Event};
use crate::model::{ClanSnap, DayPlayer, DayStats, Embed, Field, Footer};
use crate::tz::Zone;
use crate::{names, util};

/// Сколько игроков перечислять в «Кто играл».
const SHOW_PLAYERS: usize = 10;

/// Учесть событие (интересны только завершённые забеги).
pub fn record(day: &mut DayStats, ev: &Event) {
    let Event::RunFinished {
        player,
        run,
        achievements,
        build,
    } = ev
    else {
        return;
    };
    let species = if run.final_species > 0 {
        run.final_species
    } else {
        run.species
    };
    let p = day.players.entry(player.id.clone()).or_default();
    p.name.clone_from(&player.name);
    p.runs += 1;
    p.kills += run.kills.max(0);
    if run.victory {
        p.wins += 1;
    }
    if p.best_time.map_or(true, |(t, _)| run.time > t) {
        p.best_time = Some((run.time, species));
    }
    if p.best_kills.map_or(true, |(k, _)| run.kills > k) {
        p.best_kills = Some((run.kills, species));
    }
    if let Some(damage) = build.as_ref().map(|b| b.damage).filter(|d| *d > 0) {
        if p.best_damage.map_or(true, |(d, _)| damage > d) {
            p.best_damage = Some((damage, species));
        }
    }
    for a in achievements {
        match a {
            Achievement::Shinies(n) => p.shinies += n,
            Achievement::Legendaries(n) => p.legendaries += n,
            Achievement::FirstWin => p.first_win = true,
            _ => {}
        }
    }
}

/// Запомнить очки клана и участников на начало дня (если ещё не запомнили).
pub fn capture_start(day: &mut DayStats, clan: &ClanSnap) {
    if day.season_start.is_none() {
        day.season_start = clan
            .seasons
            .iter()
            .next_back()
            .map(|(n, s)| (*n, s.clone()));
    }
    if day.points_start.is_empty() {
        day.points_start = clan
            .members
            .iter()
            .map(|(id, m)| (id.clone(), m.points))
            .collect();
    }
}

/// Новый учётный период: с `now` до следующего `minute_of_day` по местному времени.
pub fn new_day(now: i64, zone: Zone, minute_of_day: i64, clan: Option<&ClanSnap>) -> DayStats {
    let mut day = DayStats {
        since: now,
        next_at: zone.next_at(now, minute_of_day),
        ..DayStats::default()
    };
    if let Some(c) = clan {
        capture_start(&mut day, c);
    }
    day
}

/// Карточка итогов. `None`, если за день никто не играл.
pub fn render(day: &DayStats, clan: Option<&ClanSnap>, zone: Zone) -> Option<Embed> {
    if day.players.is_empty() {
        return None;
    }
    let players: Vec<&DayPlayer> = day.players.values().collect();
    let runs: i64 = players.iter().map(|p| p.runs).sum();
    let wins: i64 = players.iter().map(|p| p.wins).sum();

    let (_, month, date, _) = zone.local(day.next_at.max(day.since));
    let label = clan.map(|c| clan_label(&c.tag, &c.name));
    let title = match &label {
        Some(l) => format!("📅 Итоги дня {l} · {}", util::fmt_date_ru(month, date)),
        None => format!("📅 Итоги дня · {}", util::fmt_date_ru(month, date)),
    };

    let mut lines = vec![format!(
        "🎮 Забегов: **{}** · побед: **{}** · играли: **{}**",
        util::thousands(runs),
        util::thousands(wins),
        players.len()
    )];
    if let Some(line) = clan.and_then(|c| clan_line(day, c)) {
        lines.push(line);
    }

    let mut fields = Vec::new();
    let records = records(day, clan);
    if !records.is_empty() {
        fields.push(field("🏆 Рекорды дня", records.join("\n")));
    }

    let mut active = players.clone();
    active.sort_by(|a, b| b.runs.cmp(&a.runs).then_with(|| a.name.cmp(&b.name)));
    let mut who: Vec<String> = active
        .iter()
        .take(SHOW_PLAYERS)
        .map(|p| {
            let mut s = format!("**{}** — {}", p.name, runs_word(p.runs));
            if p.wins > 0 {
                s.push_str(&format!(" · побед: {}", p.wins));
            }
            s
        })
        .collect();
    if active.len() > SHOW_PLAYERS {
        who.push(format!("и ещё {}", active.len() - SHOW_PLAYERS));
    }
    fields.push(field("👥 Кто играл", who.join("\n")));

    let notable = notable(&players);
    if !notable.is_empty() {
        fields.push(field("✨ Достижения", notable.join("\n")));
    }

    Some(Embed {
        title: Some(title),
        description: Some(lines.join("\n")),
        color: Some(0x009B_59B6),
        footer: Some(Footer {
            text: "Pokebonk · итоги дня".to_string(),
        }),
        timestamp: Some(util::iso8601(day.next_at.max(day.since))),
        fields,
        ..Embed::default()
    })
}

/// «Клан: +45 210 очков · место 741 (↑32)».
fn clan_line(day: &DayStats, clan: &ClanSnap) -> Option<String> {
    let (n, now) = clan.seasons.iter().next_back()?;
    let mut parts = Vec::new();
    match &day.season_start {
        Some((start_n, start)) if start_n == n => {
            let gained = now.points - start.points;
            if gained != 0 {
                let sign = if gained > 0 { "+" } else { "−" };
                parts.push(format!("**{sign}{}** очков", util::thousands(gained.abs())));
            }
            if let Some(rank) = now.rank {
                let change = match start.rank {
                    Some(was) if was > rank => format!(" (↑{})", was - rank),
                    Some(was) if was < rank => format!(" (↓{})", rank - was),
                    _ => String::new(),
                };
                parts.push(format!("место **{}**{change}", util::thousands(rank)));
            }
        }
        _ => {
            if let Some(rank) = now.rank {
                parts.push(format!("место **{}**", util::thousands(rank)));
            }
        }
    }
    (!parts.is_empty()).then(|| format!("🏅 Клан: {}", parts.join(" · ")))
}

fn records(day: &DayStats, clan: Option<&ClanSnap>) -> Vec<String> {
    let players: Vec<&DayPlayer> = day.players.values().collect();
    let mut out = Vec::new();

    let best = |key: &dyn Fn(&DayPlayer) -> Option<f64>| {
        players
            .iter()
            .filter_map(|p| key(p).map(|v| (v, *p)))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    };

    // Значения дня — в пределах точности f64; сравнение по f64 безопасно.
    #[allow(clippy::cast_precision_loss)]
    if let Some(p) = best(&|p| p.best_damage.map(|(d, _)| d as f64)) {
        if let Some((d, sp)) = p.best_damage {
            out.push(format!(
                "💥 Урон — **{}**: {} ({})",
                p.name,
                util::compact(d),
                names::pokemon(sp)
            ));
        }
    }
    if let Some(p) = best(&|p| p.best_time.map(|(t, _)| t)) {
        if let Some((t, sp)) = p.best_time {
            out.push(format!(
                "⏱ Дольше всех — **{}**: {} ({})",
                p.name,
                util::fmt_duration(t),
                names::pokemon(sp)
            ));
        }
    }
    #[allow(clippy::cast_precision_loss)]
    if let Some(p) = best(&|p| p.best_kills.map(|(k, _)| k as f64)) {
        if let Some((k, _)) = p.best_kills.filter(|(k, _)| *k > 0) {
            out.push(format!(
                "💀 Убийства — **{}**: {}",
                p.name,
                util::thousands(k)
            ));
        }
    }
    if let Some((name, gained)) = clan.and_then(|c| mvp(day, c)) {
        out.push(format!(
            "⭐ Больше всех очков клану — **{name}**: +{}",
            util::thousands(gained)
        ));
    }
    out
}

/// Кто принёс клану больше всего очков за день (только в пределах сезона).
fn mvp(day: &DayStats, clan: &ClanSnap) -> Option<(String, i64)> {
    let same_season = match (&day.season_start, clan.seasons.keys().next_back()) {
        (Some((start, _)), Some(now)) => start == now,
        _ => false,
    };
    if !same_season || day.points_start.is_empty() {
        return None;
    }
    clan.members
        .iter()
        .map(|(id, m)| {
            let start = day.points_start.get(id).copied().unwrap_or(m.points);
            (m.name.clone(), m.points - start)
        })
        .filter(|(_, gained)| *gained > 0)
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
}

fn notable(players: &[&DayPlayer]) -> Vec<String> {
    let list = |pick: &dyn Fn(&DayPlayer) -> i64| -> Vec<String> {
        players
            .iter()
            .filter(|p| pick(p) > 0)
            .map(|p| match pick(p) {
                1 => p.name.clone(),
                n => format!("{} ×{n}", p.name),
            })
            .collect()
    };
    let mut out = Vec::new();
    let shiny = list(&|p| p.shinies);
    if !shiny.is_empty() {
        out.push(format!("✨ Шайни: {}", shiny.join(", ")));
    }
    let legendary = list(&|p| p.legendaries);
    if !legendary.is_empty() {
        out.push(format!("👑 Легендарки: {}", legendary.join(", ")));
    }
    let first: Vec<&str> = players
        .iter()
        .filter(|p| p.first_win)
        .map(|p| p.name.as_str())
        .collect();
    if !first.is_empty() {
        out.push(format!("🥇 Первая победа: {}", first.join(", ")));
    }
    out
}

/// «1 забег», «3 забега», «12 забегов».
fn runs_word(n: i64) -> String {
    let word = match (n % 10, n % 100) {
        (1, r) if r != 11 => "забег",
        (2..=4, r) if !(12..=14).contains(&r) => "забега",
        _ => "забегов",
    };
    format!("{n} {word}")
}

fn field(name: &str, value: String) -> Field {
    Field {
        name: name.to_string(),
        value,
        inline: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::PlayerCtx;
    use crate::model::{Build, ClanSeasonSnap, MemberSnap, Run, RunBuild};

    fn run_event(id: &str, name: &str, time: f64, kills: i64, victory: bool, damage: i64) -> Event {
        Event::RunFinished {
            player: PlayerCtx {
                id: id.into(),
                name: name.into(),
                clan_tag: Some("ANP".into()),
                rank: None,
                players: None,
            },
            run: Run {
                species: 104,
                final_species: 105,
                time,
                kills,
                victory,
                ..Run::default()
            },
            achievements: if victory {
                vec![Achievement::FirstWin, Achievement::Shinies(2)]
            } else {
                vec![]
            },
            build: (damage > 0).then(|| {
                Box::new(RunBuild {
                    damage,
                    level: 10,
                    build: Some(Build::default()),
                })
            }),
        }
    }

    fn clan(points_a: i64, points_b: i64, season_points: i64, rank: i64) -> ClanSnap {
        let mut c = ClanSnap {
            id: "c1".into(),
            name: "Anapa2045".into(),
            tag: "ANP".into(),
            ..ClanSnap::default()
        };
        c.members.insert(
            "a".into(),
            MemberSnap {
                name: "pwn".into(),
                points: points_a,
                ..MemberSnap::default()
            },
        );
        c.members.insert(
            "b".into(),
            MemberSnap {
                name: "tim".into(),
                points: points_b,
                ..MemberSnap::default()
            },
        );
        c.seasons.insert(
            2,
            ClanSeasonSnap {
                points: season_points,
                rank: Some(rank),
                clans: Some(1241),
            },
        );
        c
    }

    #[test]
    fn empty_day_has_no_digest() {
        let day = DayStats::default();
        assert!(render(&day, None, Zone::Fixed(180)).is_none());
    }

    #[test]
    fn full_digest() {
        let zone = Zone::Fixed(180);
        // 23 октября 2026, 20:00 UTC = 23:00 МСК.
        let next = util::days_from_civil(2026, 10, 23) * 86_400_000 + 20 * 3_600_000;
        let mut day = DayStats {
            since: next - 86_400_000,
            next_at: next,
            ..DayStats::default()
        };
        capture_start(&mut day, &clan(1000, 500, 100_000, 773));

        record(
            &mut day,
            &run_event("a", "pwn", 2345.0, 460_059, true, 887_000_000_000),
        );
        record(&mut day, &run_event("a", "pwn", 120.0, 900, false, 0));
        record(&mut day, &run_event("b", "tim", 257.0, 1526, false, 0));

        let now = clan(21_000, 900, 120_400, 741);
        let e = render(&day, Some(&now), zone).unwrap();
        assert_eq!(e.title.as_deref(), Some("📅 Итоги дня [ANP] · 23 октября"));
        assert_eq!(
            e.description.as_deref(),
            Some(
                "🎮 Забегов: **3** · побед: **1** · играли: **2**\n\
                 🏅 Клан: **+20\u{a0}400** очков · место **741** (↑32)"
            )
        );
        assert_eq!(
            e.fields[0].value,
            "💥 Урон — **pwn**: 887B (Маровак)\n\
             ⏱ Дольше всех — **pwn**: 39:05 (Маровак)\n\
             💀 Убийства — **pwn**: 460\u{a0}059\n\
             ⭐ Больше всех очков клану — **pwn**: +20\u{a0}000"
        );
        assert_eq!(
            e.fields[1].value,
            "**pwn** — 2 забега · побед: 1\n**tim** — 1 забег"
        );
        assert_eq!(e.fields[2].value, "✨ Шайни: pwn ×2\n🥇 Первая победа: pwn");
    }

    #[test]
    fn new_season_skips_deltas() {
        let mut day = DayStats::default();
        let mut old = clan(1000, 500, 100_000, 773);
        old.seasons.clear();
        old.seasons.insert(
            1,
            ClanSeasonSnap {
                points: 5,
                rank: Some(1),
                clans: None,
            },
        );
        capture_start(&mut day, &old);
        record(&mut day, &run_event("b", "tim", 60.0, 10, false, 0));
        let e = render(&day, Some(&clan(10, 20, 30, 900)), Zone::Fixed(0)).unwrap();
        assert!(e.description.unwrap().ends_with("🏅 Клан: место **900**"));
        assert!(!e.fields[0].value.contains("очков клану"));
    }

    #[test]
    fn words() {
        assert_eq!(runs_word(1), "1 забег");
        assert_eq!(runs_word(3), "3 забега");
        assert_eq!(runs_word(11), "11 забегов");
        assert_eq!(runs_word(22), "22 забега");
        assert_eq!(runs_word(25), "25 забегов");
    }
}
