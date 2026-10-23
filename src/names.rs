//! Человекочитаемые названия: покемоны, биомы, роли в клане.

/// Покемоны первого поколения, индекс = номер в покедексе - 1.
const POKEMON_RU: [&str; 151] = [
    "Бульбазавр",
    "Ивизавр",
    "Венузавр",
    "Чармандер",
    "Чармелеон",
    "Чаризард",
    "Сквиртл",
    "Вартортл",
    "Бластойз",
    "Катерпи",
    "Метапод",
    "Баттерфри",
    "Видл",
    "Какуна",
    "Бидрилл",
    "Пиджи",
    "Пиджеотто",
    "Пиджеот",
    "Раттата",
    "Ратикейт",
    "Спироу",
    "Фироу",
    "Эканс",
    "Эрбок",
    "Пикачу",
    "Райчу",
    "Сэндшру",
    "Сэндслэш",
    "Нидоран♀",
    "Нидорина",
    "Нидоквин",
    "Нидоран♂",
    "Нидорино",
    "Нидокинг",
    "Клефэйри",
    "Клефэйбл",
    "Вульпикс",
    "Найнтейлз",
    "Джиглипафф",
    "Вигглитафф",
    "Зубат",
    "Голбат",
    "Оддиш",
    "Глум",
    "Вайлплум",
    "Парас",
    "Парасект",
    "Венонат",
    "Веномот",
    "Диглетт",
    "Дагтрио",
    "Мяут",
    "Персиан",
    "Псайдак",
    "Голдак",
    "Манки",
    "Праймейп",
    "Гроулит",
    "Арканайн",
    "Поливаг",
    "Поливирл",
    "Поливрат",
    "Абра",
    "Кадабра",
    "Алаказам",
    "Мачоп",
    "Мачоук",
    "Мачамп",
    "Беллспраут",
    "Випинбелл",
    "Виктрибел",
    "Тентакул",
    "Тентакруэль",
    "Джеодуд",
    "Гравелер",
    "Голем",
    "Понита",
    "Рапидэш",
    "Слоупок",
    "Слоубро",
    "Магнемайт",
    "Магнетон",
    "Фарфетчд",
    "Додуо",
    "Додрио",
    "Сил",
    "Дьюгонг",
    "Граймер",
    "Мак",
    "Шеллдер",
    "Клойстер",
    "Гастли",
    "Хонтер",
    "Генгар",
    "Оникс",
    "Драузи",
    "Гипно",
    "Крабби",
    "Кинглер",
    "Вольторб",
    "Электрод",
    "Экзегут",
    "Экзегутор",
    "Кьюбон",
    "Маровак",
    "Хитмонли",
    "Хитмончан",
    "Ликитунг",
    "Коффинг",
    "Визинг",
    "Райхорн",
    "Райдон",
    "Ченси",
    "Тангела",
    "Кангасхан",
    "Хорси",
    "Сидра",
    "Голдин",
    "Сикинг",
    "Старью",
    "Старми",
    "Мистер Майм",
    "Сайтер",
    "Джинкс",
    "Электабазз",
    "Магмар",
    "Пинсир",
    "Торос",
    "Мэджикарп",
    "Гаярдос",
    "Лапрас",
    "Дитто",
    "Иви",
    "Вапореон",
    "Джолтеон",
    "Флареон",
    "Поригон",
    "Оманайт",
    "Омастар",
    "Кабуто",
    "Кабутопс",
    "Аэродактиль",
    "Снорлакс",
    "Артикуно",
    "Запдос",
    "Молтрес",
    "Дратини",
    "Драгонэйр",
    "Драгонайт",
    "Мьюту",
    "Мью",
];

/// Имя покемона по номеру покедекса; для неизвестных — `#номер`.
pub fn pokemon(species: i64) -> String {
    usize::try_from(species)
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| POKEMON_RU.get(i))
        .map_or_else(|| format!("#{species}"), |name| (*name).to_string())
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

// Названия из сборки. API отдаёт коды, а русские названия игра подставляет
// сама; здесь те, что известны точно (из интерфейса игры). Остальные коды
// показываются по-английски: `life-orb` -> `Life Orb`.

const ITEMS_RU: &[(&str, &str)] = &[
    ("binding-band", "Связывающая повязка"),
    ("dusk-stone", "Сумеречный камень"),
    ("exp-share", "Делитель опыта"),
    ("light-clay", "Лёгкая глина"),
    ("lucky-egg", "Счастливое яйцо"),
    ("metronome", "Метроном"),
    ("silver-powder", "Серебряная пыль"),
    ("star-piece", "Звёздный осколок"),
];

const MOVES_RU: &[(&str, &str)] = &[
    ("signal-beam", "Сигнальный луч"),
    ("silver-wind", "Серебряный ветер"),
    ("smog", "Смог"),
    ("supersonic", "Ультразвук"),
];

const VITAMINS_RU: &[(&str, &str)] = &[
    ("elixir", "Эликсир"),
    ("pp-up", "Больше PP"),
    ("star-piece", "Звёздный осколок"),
];

const PASSIVES_RU: &[(&str, &str)] = &[("compound-eyes", "Составные глаза")];

pub fn item(code: &str) -> String {
    lookup(ITEMS_RU, code)
}

pub fn move_name(code: &str) -> String {
    lookup(MOVES_RU, code)
}

pub fn vitamin(code: &str) -> String {
    lookup(VITAMINS_RU, code)
}

pub fn passive(code: &str) -> String {
    lookup(PASSIVES_RU, code)
}

fn lookup(table: &[(&str, &str)], code: &str) -> String {
    table
        .iter()
        .find(|(k, _)| *k == code)
        .map_or_else(|| pretty_code(code), |(_, v)| (*v).to_string())
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
        assert_eq!(pokemon(152), "#152");
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
    fn build_names() {
        assert_eq!(item("lucky-egg"), "Счастливое яйцо");
        assert_eq!(item("life-orb"), "Life Orb");
        assert_eq!(item("pp-max"), "PP Max");
        assert_eq!(move_name("silver-wind"), "Серебряный ветер");
        assert_eq!(move_name("fu-t-grass"), "Fu T Grass");
        assert_eq!(vitamin("exp-candy"), "Exp Candy");
        assert_eq!(passive("photosynthesis"), "Photosynthesis");
        assert_eq!(pretty_code(""), "");
    }
}
