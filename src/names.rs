//! Человекочитаемые названия: покемоны, предметы, приёмы, биомы, роли в клане.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Словарь «код -> русское название» из перевода самой игры.
/// Пересобирается скриптом `scripts/gen-names.py`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct Names {
    pokemon: HashMap<String, String>,
    types: HashMap<String, String>,
    items: HashMap<String, String>,
    moves: HashMap<String, String>,
    abilities: HashMap<String, String>,
}

fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| {
        // Файл проверяется тестом, так что здесь он всегда корректен.
        serde_json::from_str(include_str!("names_ru.json")).unwrap_or_default()
    })
}

/// Имя покемона по номеру покедекса; для неизвестных — `#номер`.
pub fn pokemon(species: i64) -> String {
    names()
        .pokemon
        .get(&species.to_string())
        .cloned()
        .unwrap_or_else(|| format!("#{species}"))
}

/// Название биома. Неизвестные биомы показываются как есть, с заглавной буквы.
pub fn biome(code: &str) -> String {
    match code {
        "forest" => "Лес".to_string(),
        "desert" => "Пустыня".to_string(),
        "coast" => "Побережье бурь".to_string(),
        "glacier" => "Ледник".to_string(),
        "cave" => "Пещера".to_string(),
        "sea" | "ocean" => "Море".to_string(),
        "mountain" => "Горы".to_string(),
        "snow" | "tundra" => "Тундра".to_string(),
        "volcano" => "Вулкан".to_string(),
        "swamp" => "Болото".to_string(),
        "city" => "Город".to_string(),
        "" => "Неизвестный биом".to_string(),
        other => capitalize(other),
    }
}

/// Роль в клане.
pub fn role(code: &str) -> String {
    match code {
        "leader" => "лидер".to_string(),
        "officer" | "coleader" | "co-leader" => "офицер".to_string(),
        "member" => "участник".to_string(),
        other => other.to_string(),
    }
}

// Названия из сборки. Неизвестные коды (новинки игры, которых ещё нет в
// словаре) показываются по-английски: `life-orb` -> `Life Orb`.

pub fn item(code: &str) -> String {
    names()
        .items
        .get(code)
        .cloned()
        .unwrap_or_else(|| pretty_code(code))
}

/// Витамины в игре — тоже предметы.
pub fn vitamin(code: &str) -> String {
    item(code)
}

pub fn passive(code: &str) -> String {
    names()
        .abilities
        .get(code)
        .cloned()
        .unwrap_or_else(|| pretty_code(code))
}

/// Приём. Кроме обычных есть слияния игры: `fu-t-ground` — слияние типов,
/// `fu-104` — фирменное слияние покемона №104.
pub fn move_name(code: &str) -> String {
    let n = names();
    if let Some(name) = n.moves.get(code) {
        return name.clone();
    }
    if let Some(ty) = code.strip_prefix("fu-t-") {
        let ty = n.types.get(ty).cloned().unwrap_or_else(|| pretty_code(ty));
        return format!("Слияние типов: {ty}");
    }
    if let Some(num) = code.strip_prefix("fu-").and_then(|r| r.parse::<i64>().ok()) {
        return format!("Фирменное слияние: {}", pokemon(num));
    }
    pretty_code(code)
}

/// `life-orb` -> `Life Orb`, `pp-max` -> `PP Max`.
pub fn pretty_code(code: &str) -> String {
    code.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "pp" | "hp" | "xp" => w.to_uppercase(),
            _ => capitalize(w),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pokemon_names() {
        assert_eq!(pokemon(1), "Бульбазавр");
        assert_eq!(pokemon(4), "Чармандер");
        assert_eq!(pokemon(92), "Гастли");
        assert_eq!(pokemon(151), "Мью");
        assert_eq!(pokemon(0), "#0");
        assert_eq!(pokemon(152), "Chikorita"); // вне Gen 1 игра не переводит
        assert_eq!(pokemon(9999), "#9999");
        assert_eq!(pokemon(-3), "#-3");
    }

    #[test]
    fn biomes_and_roles() {
        assert_eq!(biome("forest"), "Лес");
        assert_eq!(biome("lava"), "Lava");
        assert_eq!(biome("coast"), "Побережье бурь");
        assert_eq!(role("leader"), "лидер");
        assert_eq!(role("weird"), "weird");
    }

    #[test]
    fn dictionary_is_valid() {
        let parsed: Names = serde_json::from_str(include_str!("names_ru.json")).unwrap();
        assert!(parsed.pokemon.len() >= 151);
        assert!(parsed.items.len() > 100);
        assert!(parsed.moves.len() > 500);
    }

    #[test]
    fn build_names() {
        assert_eq!(item("lucky-egg"), "Счастливое яйцо");
        assert_eq!(item("life-orb"), "Сфера жизни");
        assert_eq!(item("brand-new-thing"), "Brand New Thing");
        assert_eq!(move_name("silver-wind"), "Серебряный ветер");
        assert_eq!(move_name("razor-leaf"), "Бритвенный лист");
        assert_eq!(move_name("fu-t-ground"), "Слияние типов: Земляной");
        assert_eq!(move_name("fu-104"), "Фирменное слияние: Кубон");
        assert_eq!(move_name("fu-x"), "Fu X");
        assert_eq!(vitamin("exp-candy"), "Конфета опыта");
        assert_eq!(vitamin("calcium"), "Кальций");
        assert_eq!(passive("compound-eyes"), "Составные глаза");
        assert_eq!(passive("photosynthesis"), "Фотосинтез");
        assert_eq!(pretty_code("pp-max"), "PP Max");
        assert_eq!(pretty_code(""), "");
    }
}
