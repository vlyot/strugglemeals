"""
RecipeNLG → SQLite pipeline for StruggleMeals.

CSV columns: title, ingredients, directions, link, source, NER
- ingredients: JSON array of raw strings e.g. ["1 c. brown sugar", ...]
- NER:         JSON array of normalised names e.g. ["brown sugar", ...]

Usage:
    pip install -r requirements.txt
    python pipeline/process.py [--input data/full_dataset.csv] [--output data/recipes.db]
"""

import argparse
import json
import re
import sqlite3
import unicodedata
from pathlib import Path

import pandas as pd
from tqdm import tqdm

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

PANTRY_STAPLES: set[str] = {
    "salt", "black pepper", "white pepper", "pepper", "olive oil",
    "vegetable oil", "canola oil", "oil", "butter", "water", "sugar",
    "brown sugar", "flour", "all-purpose flour", "baking soda",
    "baking powder", "vanilla extract", "vanilla", "garlic powder",
    "onion powder", "paprika", "cumin", "oregano", "thyme", "basil",
    "cayenne", "red pepper flakes", "cinnamon", "nutmeg", "bay leaves",
    "bay leaf", "cooking spray", "nonstick cooking spray", "shortening",
}

LUXURY_BLOCKLIST: list[str] = [
    "truffle", "wagyu", "foie gras", "kobe beef", "kobe",
    "caviar", "bluefin", "sea urchin", "gold leaf", "edible gold",
    "A5", "saffron", "lobster",
]

VEGETARIAN_BLOCKLIST: list[str] = [
    "beef", "chicken", "pork", "lamb", "turkey", "veal", "bacon",
    "ham", "sausage", "fish", "shrimp", "crab", "anchovy", "tuna",
    "salmon", "gelatin", "prosciutto", "pancetta", "lard", "duck",
    "venison", "bison", "pepperoni", "salami", "chorizo", "scallop",
    "clam", "mussel", "oyster", "squid", "octopus",
]

VEGAN_EXTRA_BLOCKLIST: list[str] = [
    "milk", "cream", "cheese", "butter", "egg", "honey", "yogurt",
    "whey", "ghee", "casein", "lactose", "mayo", "mayonnaise",
]

GLUTEN_BLOCKLIST: list[str] = [
    "flour", "wheat", "barley", "rye", "pasta", "bread", "breadcrumb",
    "soy sauce", "beer", "malt", "semolina", "couscous", "bulgur",
    "farro", "spelt", "panko", "crouton", "tortilla",
]

NON_ASCII_THRESHOLD = 0.15  # fraction of non-ASCII chars to flag as non-English
MIN_CORE_INGREDIENTS = 2
MAX_CORE_INGREDIENTS = 13
MIN_STEPS = 2

SCHEMA = """
CREATE TABLE IF NOT EXISTS recipes (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    title            TEXT    NOT NULL,
    cuisine          TEXT,
    ingredients_raw  TEXT    NOT NULL,
    ingredients_core TEXT    NOT NULL,
    directions       TEXT    NOT NULL,
    ingredient_count INTEGER NOT NULL,
    vegetarian       INTEGER NOT NULL DEFAULT 0,
    vegan            INTEGER NOT NULL DEFAULT 0,
    gluten_free      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_ingredient_count ON recipes (ingredient_count);
CREATE INDEX IF NOT EXISTS idx_dietary ON recipes (vegetarian, vegan, gluten_free);
CREATE INDEX IF NOT EXISTS idx_cuisine ON recipes (cuisine);
"""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def non_ascii_ratio(text: str) -> float:
    if not text:
        return 0.0
    non_ascii = sum(1 for c in text if ord(c) > 127)
    return non_ascii / len(text)


def is_english(title: str, ingredients: list[str]) -> bool:
    combined = title + " " + " ".join(ingredients)
    return non_ascii_ratio(combined) < NON_ASCII_THRESHOLD


def parse_json_list(raw: str) -> list[str]:
    """Parse a JSON array string; return [] on failure."""
    try:
        result = json.loads(raw)
        if isinstance(result, list):
            return [str(x) for x in result]
    except (json.JSONDecodeError, TypeError):
        pass
    return []


def contains_any(tokens: list[str], blocklist: list[str]) -> bool:
    """Return True if any token in the list contains a blocklist term."""
    lowered = " ".join(tokens).lower()
    return any(term.lower() in lowered for term in blocklist)


def separate_core(ner_tokens: list[str]) -> list[str]:
    """Return NER tokens that are NOT pantry staples."""
    return [
        t for t in ner_tokens
        if t.lower() not in PANTRY_STAPLES
        and not any(t.lower() == s for s in PANTRY_STAPLES)
    ]


def tag_dietary(ner_tokens: list[str]) -> tuple[bool, bool, bool]:
    vegetarian = not contains_any(ner_tokens, VEGETARIAN_BLOCKLIST)
    vegan = vegetarian and not contains_any(ner_tokens, VEGAN_EXTRA_BLOCKLIST)
    gluten_free = not contains_any(ner_tokens, GLUTEN_BLOCKLIST)
    return vegetarian, vegan, gluten_free


def normalise_cuisine(source: str | None) -> str | None:
    if not source or not isinstance(source, str):
        return None
    s = source.strip()
    # RecipeNLG sources are domain names like "www.allrecipes.com"
    # Strip to just the domain root as a rough cuisine proxy — left nullable
    # for Phase 5 where we may do proper cuisine classification.
    return s if s else None

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main(input_path: Path, output_path: Path, chunksize: int = 50_000) -> None:
    if not input_path.exists():
        raise FileNotFoundError(
            f"Dataset not found at {input_path}.\n"
            "Download RecipeNLG from https://recipenlg.cs.put.poznan.pl/ "
            "and place the CSV at data/full_dataset.csv"
        )

    conn = sqlite3.connect(output_path)
    conn.executescript(SCHEMA)
    conn.commit()

    insert_sql = """
        INSERT INTO recipes
            (title, cuisine, ingredients_raw, ingredients_core, directions,
             ingredient_count, vegetarian, vegan, gluten_free)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """

    total_read = 0
    total_inserted = 0
    rejected = {
        "non_english": 0,
        "luxury": 0,
        "core_count": 0,
        "steps": 0,
    }

    print(f"Reading {input_path} in chunks of {chunksize:,}...")

    reader = pd.read_csv(
        input_path,
        chunksize=chunksize,
        on_bad_lines="skip",
        dtype=str,
    )

    for chunk in tqdm(reader, desc="Processing chunks", unit="chunk"):
        rows_to_insert: list[tuple] = []

        for _, row in chunk.iterrows():
            total_read += 1

            title = str(row.get("title", "") or "").strip()
            raw_ingredients = parse_json_list(row.get("ingredients", "[]"))
            raw_directions = parse_json_list(row.get("directions", "[]"))
            ner_tokens = parse_json_list(row.get("NER", "[]"))
            source = row.get("source")

            # --- Filter: non-English ---
            if not is_english(title, ner_tokens):
                rejected["non_english"] += 1
                continue

            # --- Filter: luxury ---
            if contains_any(ner_tokens, LUXURY_BLOCKLIST):
                rejected["luxury"] += 1
                continue

            # --- Separate core from pantry ---
            core_tokens = separate_core(ner_tokens)

            # --- Filter: core ingredient count ---
            core_count = len(core_tokens)
            if core_count < MIN_CORE_INGREDIENTS or core_count > MAX_CORE_INGREDIENTS:
                rejected["core_count"] += 1
                continue

            # --- Filter: minimum steps ---
            if len(raw_directions) < MIN_STEPS:
                rejected["steps"] += 1
                continue

            # --- Dietary tagging (on full NER list, not core-only) ---
            vegetarian, vegan, gluten_free = tag_dietary(ner_tokens)

            cuisine = normalise_cuisine(source)

            rows_to_insert.append((
                title,
                cuisine,
                json.dumps(raw_ingredients),
                json.dumps(core_tokens),
                json.dumps(raw_directions),
                core_count,
                int(vegetarian),
                int(vegan),
                int(gluten_free),
            ))

        if rows_to_insert:
            conn.executemany(insert_sql, rows_to_insert)
            conn.commit()
            total_inserted += len(rows_to_insert)

    conn.close()

    print(f"\nDone.")
    print(f"  Rows read:     {total_read:>10,}")
    print(f"  Rows inserted: {total_inserted:>10,}")
    print(f"  Rejected:")
    for reason, count in rejected.items():
        print(f"    {reason:<18} {count:>10,}")
    print(f"\nDatabase written to: {output_path}")
    print(f"File size: {output_path.stat().st_size / 1_048_576:.1f} MB")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Process RecipeNLG CSV into SQLite")
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("data/full_dataset.csv"),
        help="Path to RecipeNLG CSV (default: data/full_dataset.csv)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("data/recipes.db"),
        help="Output SQLite path (default: data/recipes.db)",
    )
    parser.add_argument(
        "--chunksize",
        type=int,
        default=50_000,
        help="Rows per chunk (default: 50000)",
    )
    args = parser.parse_args()
    main(args.input, args.output, args.chunksize)
