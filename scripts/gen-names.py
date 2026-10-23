#!/usr/bin/env python3
"""Собирает src/names_ru.json — словарь «код из API -> русское название».

Источники:
  * перевод самой игры: https://pokebonk.com/data/i18n/ru.json
    (покемоны и предметы — по коду, приёмы и способности — по номеру PokeAPI);
  * таблицы PokeAPI «код -> номер» для приёмов и способностей.

Когда в игре появятся новые предметы или приёмы:
    python scripts/gen-names.py
    git commit -am "names: update"

Свои файлы (без скачивания): --game ru.json --csv-dir путь/к/pokeapi/data/v2/csv
"""
from __future__ import annotations

import argparse
import csv
import io
import json
import pathlib
import urllib.request

GAME_URL = "https://pokebonk.com/data/i18n/ru.json"
CSV_URL = "https://raw.githubusercontent.com/PokeAPI/pokeapi/master/data/v2/csv/{}.csv"
OUT = pathlib.Path(__file__).resolve().parent.parent / "src" / "names_ru.json"

# Кастомные коды игры, которых нет в её ru.json по коду; русские названия
# взяты из её же перевода интерфейса (assets/ru-*.js).
EXTRA = {
    "items": {
        "exp-candy": "Конфета опыта",
        "cursed": "Проклятый",
    },
    "abilities": {
        "photosynthesis": "Фотосинтез",
        "bone-keeper": "Кость хранителя",
    },
}


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": "pokebonk-lolka gen-names"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read().decode("utf-8")


def ids(name: str, csv_dir: pathlib.Path | None) -> dict[str, str]:
    """identifier -> id из таблицы PokeAPI."""
    text = (csv_dir / f"{name}.csv").read_text("utf-8") if csv_dir else fetch(CSV_URL.format(name))
    return {row["identifier"]: row["id"] for row in csv.DictReader(io.StringIO(text))}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--game", help="локальная копия ru.json игры")
    ap.add_argument("--csv-dir", type=pathlib.Path, help="папка с moves.csv и abilities.csv PokeAPI")
    args = ap.parse_args()

    game = json.loads(pathlib.Path(args.game).read_text("utf-8") if args.game else fetch(GAME_URL))

    def name(v):
        return v["name"] if isinstance(v, dict) else v

    out = {
        "pokemon": {k: name(v) for k, v in game.get("pokemon", {}).items()},
        "types": {k: name(v) for k, v in game.get("types", {}).items()},
        "items": {k: name(v) for k, v in game.get("items", {}).items()},
        "moves": {},
        "abilities": {},
    }
    for section, table in (("moves", "moves"), ("abilities", "abilities")):
        by_id = game.get(section, {})
        for slug, num in ids(table, args.csv_dir).items():
            if num in by_id:
                out[section][slug] = name(by_id[num])
    for section, extra in EXTRA.items():
        for k, v in extra.items():
            out[section].setdefault(k, v)

    # Компактно и стабильно: одна запись на строку, ключи по алфавиту.
    lines = ["{"]
    sections = list(out)
    for i, section in enumerate(sections):
        lines.append(f'"{section}":{{')
        items = sorted(out[section].items(), key=lambda kv: (len(kv[0]), kv[0]) if section == "pokemon" else kv[0])
        for j, (k, v) in enumerate(items):
            comma = "," if j < len(items) - 1 else ""
            lines.append(f"{json.dumps(k, ensure_ascii=False)}:{json.dumps(v, ensure_ascii=False)}{comma}")
        lines.append("}" + ("," if i < len(sections) - 1 else ""))
    lines.append("}")
    OUT.write_text("\n".join(lines) + "\n", "utf-8")
    print(OUT, {k: len(v) for k, v in out.items()})


if __name__ == "__main__":
    main()
