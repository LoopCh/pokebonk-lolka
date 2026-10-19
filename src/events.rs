//! Чистая логика: сравниваем прошлый снимок с новым ответом API и решаем, о чём
//! сообщить. Здесь нет сети и файлов, поэтому всё легко покрыть тестами.

use crate::model::{
    Clan, ClanSeasonSnap, ClanSnap, MemberSnap, Player, PlayerSnap, Run, SeasonInfo, SeasonSnap,
    Stats,
};
use std::collections::BTreeMap;

/// Сколько последних сезонов клана помнить.
const KEEP_SEASONS: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum Achievement {
    FirstWin,
    Shinies(i64),
    Legendaries(i64),
    TimeMedals(i64),
    Caught(i64),
    MaxLevel(i64),
    MaxTime(f64),
}

/// Кто сыграл забег: то, что нужно для карточки.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerCtx {
    pub id: String,
    pub name: String,
    pub clan_tag: Option<String>,
    pub rank: Option<i64>,
    pub players: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    RunFinished {
        player: PlayerCtx,
        run: Run,
        achievements: Vec<Achievement>,
    },
    Renamed {
        old: String,
        new: String,
    },
    Joined {
        name: String,
        clan: String,
    },
    Left {
        name: String,
        clan: String,
    },
    RoleChanged {
        name: String,
        old: String,
        new: String,
    },
    SeasonEndingSoon {
        name: String,
        end_ms: i64,
        now_ms: i64,
    },
    SeasonStarted {
        name: String,
        end_ms: Option<i64>,
        now_ms: i64,
    },
    SeasonFinished {
        name: String,
        clan: Option<(String, ClanSeasonSnap)>,
    },
    Announcement {
        text: String,
    },
}

// ---------------------------------------------------------------------------
// Игрок
// ---------------------------------------------------------------------------

/// Сравнивает игрока с прошлым снимком. Если снимка нет (игрока только что
/// начали отслеживать), событий нет: запоминаем текущее состояние как базовое,
/// чтобы не заспамить канал старыми забегами.
pub fn player_update(prev: Option<&PlayerSnap>, p: &Player) -> (Vec<Event>, PlayerSnap) {
    let latest = p.recent.iter().map(|r| r.date).max().unwrap_or(0);
    let Some(prev) = prev else {
        let snap = PlayerSnap {
            name: p.name.clone(),
            last_run_date: latest,
            stats: p.stats.clone(),
        };
        return (Vec::new(), snap);
    };

    let mut events = Vec::new();
    if !prev.name.is_empty() && !p.name.is_empty() && prev.name != p.name {
        events.push(Event::Renamed {
            old: prev.name.clone(),
            new: p.name.clone(),
        });
    }

    let mut new_runs: Vec<&Run> = p
        .recent
        .iter()
        .filter(|r| r.date > prev.last_run_date)
        .collect();
    new_runs.sort_by_key(|r| r.date);

    if let Some((last, earlier)) = new_runs.split_last() {
        let ctx = PlayerCtx {
            id: p.id.clone(),
            name: p.name.clone(),
            clan_tag: p
                .clan
                .as_ref()
                .map(|c| c.tag.clone())
                .filter(|t| !t.is_empty()),
            rank: p.standing.as_ref().and_then(|s| s.rank),
            players: p.standing.as_ref().and_then(|s| s.players),
        };
        for run in earlier {
            events.push(Event::RunFinished {
                player: ctx.clone(),
                run: (*run).clone(),
                achievements: Vec::new(),
            });
        }
        // Статистика общая, поэтому достижения приписываем последнему забегу.
        events.push(Event::RunFinished {
            player: ctx,
            run: (*last).clone(),
            achievements: achievements(&prev.stats, &p.stats),
        });
    }

    let snap = PlayerSnap {
        name: p.name.clone(),
        last_run_date: latest.max(prev.last_run_date),
        stats: p.stats.clone(),
    };
    (events, snap)
}

/// Что изменилось в статистике между двумя снимками.
pub fn achievements(old: &Stats, new: &Stats) -> Vec<Achievement> {
    let mut out = Vec::new();
    if old.wins == 0 && new.wins > 0 {
        out.push(Achievement::FirstWin);
    }
    if new.shinies > old.shinies {
        out.push(Achievement::Shinies(new.shinies - old.shinies));
    }
    if new.legendaries > old.legendaries {
        out.push(Achievement::Legendaries(new.legendaries - old.legendaries));
    }
    if new.time_medals > old.time_medals {
        out.push(Achievement::TimeMedals(new.time_medals - old.time_medals));
    }
    if new.caught > old.caught {
        out.push(Achievement::Caught(new.caught - old.caught));
    }
    // Рекорды показываем, только если старый рекорд уже был, иначе первый же
    // забег новичка был бы «рекордным».
    if old.max_level > 0 && new.max_level > old.max_level {
        out.push(Achievement::MaxLevel(new.max_level));
    }
    if old.max_time > 0.0 && new.max_time > old.max_time + 0.5 {
        out.push(Achievement::MaxTime(new.max_time));
    }
    out
}

// ---------------------------------------------------------------------------
// Клан
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ClanDiff {
    pub events: Vec<Event>,
    pub snap: ClanSnap,
    /// Игроки, которых стоит проверить сейчас: вырос счётчик забегов или новичок.
    pub check: Vec<String>,
}

/// Сравнивает клан с прошлым снимком. Первый снимок (или другой клан) —
/// базовый: событий нет, проверяем всех участников, чтобы запомнить их забеги.
pub fn clan_update(prev: Option<&ClanSnap>, c: &Clan) -> ClanDiff {
    let members: BTreeMap<String, MemberSnap> = c
        .members
        .iter()
        .filter(|m| !m.id.is_empty())
        .map(|m| {
            let snap = MemberSnap {
                name: m.name.clone(),
                role: m.role.clone(),
                runs: m.runs,
            };
            (m.id.clone(), snap)
        })
        .collect();

    let same_clan = prev.filter(|p| p.id == c.id);
    let mut seasons = same_clan.map(|p| p.seasons.clone()).unwrap_or_default();
    if let Some(s) = &c.season {
        let snap = ClanSeasonSnap {
            points: s.points,
            rank: s.rank,
            clans: s.clans,
        };
        seasons.insert(s.n, snap);
        while seasons.len() > KEEP_SEASONS {
            seasons.pop_first();
        }
    }

    let tag = clan_label(&c.tag, &c.name);
    let mut events = Vec::new();
    let mut check = Vec::new();

    match same_clan {
        None => check.extend(members.keys().cloned()),
        Some(p) => {
            for (id, m) in &members {
                match p.members.get(id) {
                    None => {
                        events.push(Event::Joined {
                            name: m.name.clone(),
                            clan: tag.clone(),
                        });
                        check.push(id.clone());
                    }
                    Some(old) => {
                        if m.runs > old.runs {
                            check.push(id.clone());
                        }
                        if !old.role.is_empty() && !m.role.is_empty() && old.role != m.role {
                            events.push(Event::RoleChanged {
                                name: m.name.clone(),
                                old: old.role.clone(),
                                new: m.role.clone(),
                            });
                        }
                    }
                }
            }
            for (id, old) in &p.members {
                if !members.contains_key(id) {
                    events.push(Event::Left {
                        name: old.name.clone(),
                        clan: tag.clone(),
                    });
                }
            }
        }
    }

    let snap = ClanSnap {
        id: c.id.clone(),
        name: c.name.clone(),
        tag: c.tag.clone(),
        members,
        seasons,
    };
    ClanDiff {
        events,
        snap,
        check,
    }
}

/// `[ANP]`, а если тега нет — имя клана.
pub fn clan_label(tag: &str, name: &str) -> String {
    if tag.is_empty() {
        name.to_string()
    } else {
        format!("[{tag}]")
    }
}

// ---------------------------------------------------------------------------
// Сезон
// ---------------------------------------------------------------------------

/// Следит за сезоном: смена сезона, напоминание о скором конце, объявления.
pub fn season_update(
    prev: &SeasonSnap,
    info: &SeasonInfo,
    now_ms: i64,
    remind_ms: i64,
    clan: Option<&ClanSnap>,
) -> (Vec<Event>, SeasonSnap) {
    let end = info.end.unwrap_or(0);
    let message = info.message.trim().to_string();

    if prev.n == 0 {
        // Первый запуск: просто запоминаем. Напоминание ещё может сработать.
        let mut snap = SeasonSnap {
            n: info.n,
            name: info.name.clone(),
            end,
            reminded: false,
            message,
        };
        let events = remind(&mut snap, info.live, now_ms, remind_ms);
        return (events, snap);
    }

    let mut events = Vec::new();
    let mut snap = prev.clone();

    if info.n != prev.n {
        let clan_result = clan.and_then(|c| {
            c.seasons
                .get(&prev.n)
                .map(|s| (clan_label(&c.tag, &c.name), s.clone()))
        });
        events.push(Event::SeasonFinished {
            name: prev.name.clone(),
            clan: clan_result,
        });
        events.push(Event::SeasonStarted {
            name: info.name.clone(),
            end_ms: (end > 0).then_some(end),
            now_ms,
        });
        snap.n = info.n;
        snap.reminded = false;
    }
    if end != snap.end {
        // Сезон продлили или сократили — напомним заново.
        snap.reminded = false;
    }
    snap.name.clone_from(&info.name);
    snap.end = end;

    events.extend(remind(&mut snap, info.live, now_ms, remind_ms));

    if message != snap.message {
        if !message.is_empty() {
            events.push(Event::Announcement {
                text: message.clone(),
            });
        }
        snap.message = message;
    }
    (events, snap)
}

fn remind(snap: &mut SeasonSnap, live: bool, now_ms: i64, remind_ms: i64) -> Vec<Event> {
    let due = live
        && remind_ms > 0
        && !snap.reminded
        && snap.end > now_ms
        && snap.end - now_ms <= remind_ms;
    if !due {
        return Vec::new();
    }
    snap.reminded = true;
    vec![Event::SeasonEndingSoon {
        name: snap.name.clone(),
        end_ms: snap.end,
        now_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClanRef, ClanSeason, Member, Standing};

    fn run(date: i64, victory: bool) -> Run {
        Run {
            date,
            species: 1,
            final_species: 3,
            biome: "forest".into(),
            victory,
            ..Run::default()
        }
    }

    fn player(recent: Vec<Run>, stats: Stats) -> Player {
        Player {
            id: "p1".into(),
            name: "ash".into(),
            stats,
            recent,
            clan: Some(ClanRef { tag: "ANP".into() }),
            standing: Some(Standing {
                rank: Some(10),
                players: Some(100),
            }),
        }
    }

    #[test]
    fn first_sight_is_silent_baseline() {
        let p = player(vec![run(300, false), run(100, true)], Stats::default());
        let (events, snap) = player_update(None, &p);
        assert!(events.is_empty());
        assert_eq!(snap.last_run_date, 300);
    }

    #[test]
    fn new_runs_are_reported_oldest_first_with_achievements_on_last() {
        let prev = PlayerSnap {
            name: "ash".into(),
            last_run_date: 100,
            stats: Stats {
                max_level: 50,
                ..Stats::default()
            },
        };
        let stats = Stats {
            wins: 1,
            shinies: 2,
            max_level: 60,
            ..Stats::default()
        };
        let p = player(
            vec![run(300, true), run(200, false), run(100, false)],
            stats,
        );
        let (events, snap) = player_update(Some(&prev), &p);
        assert_eq!(snap.last_run_date, 300);
        assert_eq!(events.len(), 2);

        let Event::RunFinished {
            run, achievements, ..
        } = &events[0]
        else {
            panic!("expected run");
        };
        assert_eq!(run.date, 200);
        assert!(achievements.is_empty());

        let Event::RunFinished {
            run,
            achievements,
            player,
        } = &events[1]
        else {
            panic!("expected run");
        };
        assert_eq!(run.date, 300);
        assert_eq!(player.clan_tag.as_deref(), Some("ANP"));
        assert_eq!(
            achievements,
            &vec![
                Achievement::FirstWin,
                Achievement::Shinies(2),
                Achievement::MaxLevel(60)
            ]
        );
    }

    #[test]
    fn nothing_new_means_no_events() {
        let prev = PlayerSnap {
            name: "ash".into(),
            last_run_date: 300,
            stats: Stats::default(),
        };
        let p = player(vec![run(300, true)], Stats::default());
        let (events, _) = player_update(Some(&prev), &p);
        assert!(events.is_empty());
    }

    #[test]
    fn empty_recent_keeps_last_date() {
        let prev = PlayerSnap {
            name: "ash".into(),
            last_run_date: 300,
            stats: Stats::default(),
        };
        let (_, snap) = player_update(Some(&prev), &player(vec![], Stats::default()));
        assert_eq!(snap.last_run_date, 300);
    }

    #[test]
    fn rename_is_reported() {
        let prev = PlayerSnap {
            name: "old".into(),
            last_run_date: 300,
            stats: Stats::default(),
        };
        let (events, snap) = player_update(Some(&prev), &player(vec![], Stats::default()));
        assert_eq!(
            events,
            vec![Event::Renamed {
                old: "old".into(),
                new: "ash".into()
            }]
        );
        assert_eq!(snap.name, "ash");
    }

    #[test]
    fn records_need_a_previous_record() {
        let old = Stats::default();
        let new = Stats {
            max_level: 10,
            max_time: 100.0,
            ..Stats::default()
        };
        assert!(achievements(&old, &new).is_empty());
    }

    fn member(id: &str, role: &str, runs: i64) -> Member {
        Member {
            id: id.into(),
            name: format!("n-{id}"),
            role: role.into(),
            runs,
        }
    }

    fn clan(members: Vec<Member>, season_n: i64) -> Clan {
        Clan {
            id: "c1".into(),
            name: "Team".into(),
            tag: "ANP".into(),
            members,
            season: Some(ClanSeason {
                n: season_n,
                points: 1000 * season_n,
                rank: Some(7),
                clans: Some(50),
            }),
        }
    }

    #[test]
    fn clan_baseline_checks_everyone_silently() {
        let c = clan(vec![member("a", "leader", 1), member("b", "member", 0)], 1);
        let d = clan_update(None, &c);
        assert!(d.events.is_empty());
        assert_eq!(d.check, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(d.snap.members.len(), 2);
    }

    #[test]
    fn clan_changes() {
        let before = clan_update(
            None,
            &clan(
                vec![
                    member("a", "leader", 1),
                    member("b", "member", 0),
                    member("c", "member", 5),
                ],
                1,
            ),
        )
        .snap;
        let now = clan(
            vec![
                member("a", "leader", 2),  // сыграл
                member("b", "officer", 0), // повышен
                member("d", "member", 0),  // новичок
            ],
            1,
        );
        let d = clan_update(Some(&before), &now);
        assert_eq!(d.check, vec!["a".to_string(), "d".to_string()]);
        assert_eq!(
            d.events,
            vec![
                Event::RoleChanged {
                    name: "n-b".into(),
                    old: "member".into(),
                    new: "officer".into()
                },
                Event::Joined {
                    name: "n-d".into(),
                    clan: "[ANP]".into()
                },
                Event::Left {
                    name: "n-c".into(),
                    clan: "[ANP]".into()
                },
            ]
        );
    }

    #[test]
    fn clan_keeps_recent_seasons() {
        let mut snap = clan_update(None, &clan(vec![member("a", "leader", 0)], 1)).snap;
        for n in 2..=5 {
            snap = clan_update(Some(&snap), &clan(vec![member("a", "leader", 0)], n)).snap;
        }
        assert_eq!(
            snap.seasons.keys().copied().collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
    }

    fn info(n: i64, end: i64, message: &str) -> SeasonInfo {
        SeasonInfo {
            n,
            name: format!("GEN {n}"),
            end: Some(end),
            live: true,
            message: message.into(),
        }
    }

    const HOUR: i64 = 3_600_000;

    #[test]
    fn season_first_sight_and_reminder() {
        let (events, snap) = season_update(
            &SeasonSnap::default(),
            &info(1, 100 * HOUR, ""),
            0,
            24 * HOUR,
            None,
        );
        assert!(events.is_empty());
        assert_eq!(snap.n, 1);

        // За 24 часа до конца — одно напоминание.
        let now = 80 * HOUR;
        let (events, snap) = season_update(&snap, &info(1, 100 * HOUR, ""), now, 24 * HOUR, None);
        assert_eq!(
            events,
            vec![Event::SeasonEndingSoon {
                name: "GEN 1".into(),
                end_ms: 100 * HOUR,
                now_ms: now
            }]
        );
        let (events, _) = season_update(&snap, &info(1, 100 * HOUR, ""), now + 1, 24 * HOUR, None);
        assert!(events.is_empty());
    }

    #[test]
    fn season_rollover_reports_clan_result() {
        let clan_snap = clan_update(None, &clan(vec![member("a", "leader", 0)], 1)).snap;
        let prev = SeasonSnap {
            n: 1,
            name: "GEN 1".into(),
            end: 100 * HOUR,
            reminded: true,
            message: String::new(),
        };
        let (events, snap) = season_update(
            &prev,
            &info(2, 300 * HOUR, ""),
            101 * HOUR,
            24 * HOUR,
            Some(&clan_snap),
        );
        assert_eq!(snap.n, 2);
        assert!(!snap.reminded);
        assert_eq!(
            events[0],
            Event::SeasonFinished {
                name: "GEN 1".into(),
                clan: Some((
                    "[ANP]".into(),
                    ClanSeasonSnap {
                        points: 1000,
                        rank: Some(7),
                        clans: Some(50)
                    }
                ))
            }
        );
        assert!(matches!(events[1], Event::SeasonStarted { .. }));
    }

    #[test]
    fn announcements_only_when_message_changes() {
        let prev = SeasonSnap {
            n: 1,
            name: "GEN 1".into(),
            end: 100 * HOUR,
            reminded: false,
            message: String::new(),
        };
        let (events, snap) = season_update(&prev, &info(1, 100 * HOUR, " Patch 1.2! "), 0, 0, None);
        assert_eq!(
            events,
            vec![Event::Announcement {
                text: "Patch 1.2!".into()
            }]
        );
        let (events, _) = season_update(&snap, &info(1, 100 * HOUR, "Patch 1.2!"), 0, 0, None);
        assert!(events.is_empty());
    }
}
