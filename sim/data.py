"""Loads content from /data. Content is data, never code."""
from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path

DATA_DIR = Path(__file__).resolve().parent.parent / "data"


@lru_cache(maxsize=None)
def _load(name: str) -> dict:
    with open(DATA_DIR / name, encoding="utf-8") as f:
        return json.load(f)


def races() -> dict:
    return _load("races.json")


def items() -> dict:
    return _load("items.json")["items"]


def consumables() -> dict:
    return _load("items.json")["consumables"]


def gold_values() -> dict:
    return _load("items.json")["gold_values"]


def spells() -> dict:
    return _load("spells.json")["spells"]


def jobs() -> dict:
    return _load("jobs.json")
