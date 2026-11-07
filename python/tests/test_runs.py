"""Smoke tests: both simulations run from Python and a seed reproduces them."""

from pathlib import Path

import caspers
import pyarrow.parquet as pq


def logical_events(path: str) -> list[dict]:
    """Events without their CloudEvents ids, which are random per run."""
    table = pq.read_table(path)
    return table.drop_columns(["id"]).to_pylist()


def test_kitchen_seed_reproduces_events(tmp_path: Path) -> None:
    runs = [
        caspers.run_kitchen_simulation(steps=120, output_dir=str(tmp_path / name), seed=7)
        for name in ("a", "b")
    ]
    first, second = (logical_events(r.events_path) for r in runs)
    assert len(first) > 20
    assert first == second
    assert first[0]["time"].startswith("2026-01-05T11:")
    assert (tmp_path / "a" / "artifacts" / "structured" / "payment_authorization.jsonl").exists()


def test_logistics_seed_reproduces_events(tmp_path: Path) -> None:
    runs = [
        caspers.run_logistics_simulation(steps=480, output_dir=str(tmp_path / name), seed=7)
        for name in ("a", "b")
    ]
    assert runs[0].packages_delivered > 0
    assert runs[0].packages_delivered == runs[1].packages_delivered
    first, second = (logical_events(r.events_path) for r in runs)
    assert first == second


def test_different_seeds_diverge(tmp_path: Path) -> None:
    a = caspers.run_kitchen_simulation(steps=120, output_dir=str(tmp_path / "a"), seed=1)
    b = caspers.run_kitchen_simulation(steps=120, output_dir=str(tmp_path / "b"), seed=2)
    assert logical_events(a.events_path) != logical_events(b.events_path)
