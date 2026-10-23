//! Новости игры (`GET /news`) для отдельного канала.

use crate::model::{Embed, Footer, NewsItem, NewsText};
use crate::util;

/// Lolka: до 4096 символов в описании; оставляем запас на заголовок раздела.
const MAX_DESCRIPTION: usize = 3800;
const SITE: &str = "https://pokebonk.com/";

/// Новые новости, старые сначала. При первом запуске (`seen` = None)
/// новостей нет: всё, что уже есть, запоминается как прочитанное.
pub fn new_items(seen: Option<i64>, items: &[NewsItem]) -> (Vec<&NewsItem>, Option<i64>) {
    let max = items.iter().map(|n| n.id).max();
    let Some(seen) = seen else {
        return (Vec::new(), max.or(Some(0)));
    };
    let mut fresh: Vec<&NewsItem> = items.iter().filter(|n| n.id > seen).collect();
    fresh.sort_by_key(|n| n.id);
    (fresh, Some(max.map_or(seen, |m| m.max(seen))))
}

/// Текст на нужном языке: русский, иначе английский, иначе испанский оригинал.
fn localized(item: &NewsItem) -> &NewsText {
    ["ru", "en"]
        .iter()
        .find_map(|lang| item.tr.get(*lang).filter(|t| !t.title.is_empty()))
        .unwrap_or(&item.text)
}

pub fn render(item: &NewsItem) -> Embed {
    let text = localized(item);
    let (emoji, color) = match item.tag.as_str() {
        "parche" => ("🛠", 0x00E6_7E22),
        "actualizacion" => ("🆕", 0x0057_F287),
        "novedad" => ("📰", 0x0034_98DB),
        _ => ("📣", 0x009B_59B6),
    };

    let mut description = String::new();
    if !text.summary.trim().is_empty() {
        description.push_str(&format!("*{}*\n\n", text.summary.trim()));
    }
    let body = to_lolka_markdown(&text.body, &text.title);
    description.push_str(&fit(
        &body,
        MAX_DESCRIPTION.saturating_sub(description.chars().count()),
    ));

    Embed {
        title: Some(util::truncate(
            &format!("{emoji} {}", text.title.trim()),
            256,
        )),
        description: Some(description.trim().to_string()),
        url: Some(SITE.to_string()),
        color: Some(color),
        timestamp: (item.date > 0).then(|| util::iso8601(item.date)),
        footer: Some(Footer {
            text: "PokéBonk · новости".to_string(),
        }),
        ..Embed::default()
    }
}

/// Разметка игры -> то, что понимает Lolka (жирный, курсив, цитаты, ссылки):
/// заголовки становятся жирными, блоки `:::nuevo` — цитатами со значком,
/// иконки `{pkm:…}` / `{obj:…}` убираются.
pub fn to_lolka_markdown(body: &str, title: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false;
    // Значок блока ждёт первую строку, чтобы встать с ней в одну строку.
    let mut badge: Option<&str> = None;
    let mut started = false;
    for raw in body.lines() {
        let line = strip_icons(raw);
        let trimmed = line.trim();
        let first = !started && !trimmed.is_empty();
        started |= first;

        if let Some(kind) = trimmed.strip_prefix(":::") {
            if in_block && kind.is_empty() {
                in_block = false;
                if let Some(b) = badge.take() {
                    out.push(format!("> {b}"));
                }
            } else {
                in_block = true;
                badge = Some(match kind {
                    "nuevo" => "🆕",
                    "aviso" => "⚠️",
                    "consejo" => "💡",
                    _ => "ℹ️",
                });
            }
            continue;
        }
        if trimmed == "---" {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix('#') {
            let heading = heading.trim_start_matches('#').trim();
            // Первый заголовок обычно повторяет начало названия новости.
            if first && !heading.is_empty() && title.trim().starts_with(heading) {
                continue;
            }
            if !heading.is_empty() {
                out.push(String::new());
                out.push(format!("**{heading}**"));
            }
            continue;
        }
        if in_block {
            if trimmed.is_empty() {
                if badge.is_none() {
                    out.push(">".to_string());
                }
            } else if let Some(b) = badge.take() {
                out.push(format!("> {b} {trimmed}"));
            } else {
                out.push(format!("> {trimmed}"));
            }
            continue;
        }
        out.push(line.trim_end().to_string());
    }

    // Не больше одной пустой строки подряд.
    let mut text = String::new();
    let mut blank = 0;
    for line in out {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        text.push_str(&line);
        text.push('\n');
    }
    text.trim().to_string()
}

/// Убирает иконки `{pkm:145}`, `{obj:rare-candy}` и лишние пробелы после них.
fn strip_icons(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find('{') {
        let (before, after) = rest.split_at(start);
        out.push_str(before);
        let icon = after.find('}').filter(|end| {
            let inner = &after[1..*end];
            inner.split_once(':').is_some_and(|(kind, value)| {
                !kind.is_empty()
                    && kind.chars().all(|c| c.is_ascii_lowercase())
                    && !value.is_empty()
                    && !value.contains(char::is_whitespace)
            })
        });
        match icon {
            Some(end) => {
                rest = after[end + 1..].trim_start_matches(' ');
            }
            None => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Обрезает по границе строки, чтобы не рвать разметку посреди слова.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let note = "\n\n…полностью — в игре, раздел «Новости».";
    let budget = max.saturating_sub(note.chars().count());
    let mut out = String::new();
    for line in text.lines() {
        if out.chars().count() + line.chars().count() + 1 > budget {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out.trim().is_empty() {
        out = util::truncate(text, budget);
    }
    format!("{}{note}", out.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn item(id: i64) -> NewsItem {
        NewsItem {
            id,
            ..NewsItem::default()
        }
    }

    #[test]
    fn first_run_is_silent() {
        let items = vec![item(3), item(5), item(4)];
        let (fresh, seen) = new_items(None, &items);
        assert!(fresh.is_empty());
        assert_eq!(seen, Some(5));

        let (fresh, seen) = new_items(Some(3), &items);
        assert_eq!(fresh.iter().map(|n| n.id).collect::<Vec<_>>(), vec![4, 5]);
        assert_eq!(seen, Some(5));

        let (fresh, seen) = new_items(Some(7), &[]);
        assert!(fresh.is_empty());
        assert_eq!(seen, Some(7));
    }

    #[test]
    fn markdown_conversion() {
        let body = "# Сезон 2 уже здесь!\n\n:::nuevo\n**Сезон 2** стартует в среду.\nВсе стартуем вместе.\n:::\n\n---\n\n## {pkm:145} Побережье бурь\n\nКарта с морем.\n\n- **Прилив** затапливает пляжи.\n\n\n\n## {obj:rare-candy} 5 новых витаминов\nТекст {с фигурными} скобками.";
        let md = to_lolka_markdown(body, "Сезон 2 уже здесь! Две новые карты");
        assert_eq!(
            md,
            "> 🆕 **Сезон 2** стартует в среду.\n\
             > Все стартуем вместе.\n\
             \n\
             **Побережье бурь**\n\
             \n\
             Карта с морем.\n\
             \n\
             - **Прилив** затапливает пляжи.\n\
             \n\
             **5 новых витаминов**\n\
             Текст {с фигурными} скобками."
        );
    }

    #[test]
    fn render_prefers_russian_and_fits_limits() {
        let mut tr = BTreeMap::new();
        tr.insert(
            "ru".to_string(),
            NewsText {
                title: "Патч №7".into(),
                summary: "Карта ведёт тебя.".into(),
                body: format!("# Патч №7\n\n{}", "- исправление\n".repeat(2000)),
            },
        );
        let n = NewsItem {
            id: 12,
            text: NewsText {
                title: "Parche 7".into(),
                ..NewsText::default()
            },
            tag: "parche".into(),
            date: 1_792_594_450_380,
            tr,
        };
        let e = render(&n);
        assert_eq!(e.title.as_deref(), Some("🛠 Патч №7"));
        let d = e.description.unwrap();
        assert!(d.starts_with("*Карта ведёт тебя.*\n\n- исправление"));
        assert!(d.ends_with("…полностью — в игре, раздел «Новости»."));
        assert!(d.chars().count() <= MAX_DESCRIPTION);

        // Нет перевода — испанский оригинал.
        let es = NewsItem {
            tr: BTreeMap::new(),
            ..n
        };
        assert_eq!(render(&es).title.as_deref(), Some("🛠 Parche 7"));
    }

    #[test]
    fn parses_real_news_shape() {
        let json = r##"[{"id":15,"title":"¡Bienvenidos!","summary":"Resumen","body":"# x","tag":"novedad","cover":"","pinned":true,"date":1792594450380,"updated":1792594450380,"tr":{"ru":{"body":"# Добро пожаловать","title":"Добро пожаловать в Сезон 2!","summary":"Коротко"}}}]"##;
        let items: Vec<NewsItem> = serde_json::from_str(json).unwrap();
        assert_eq!(items[0].id, 15);
        assert_eq!(items[0].text.title, "¡Bienvenidos!");
        assert_eq!(items[0].tr["ru"].summary, "Коротко");
        assert_eq!(
            render(&items[0]).title.as_deref(),
            Some("📰 Добро пожаловать в Сезон 2!")
        );
    }
}
