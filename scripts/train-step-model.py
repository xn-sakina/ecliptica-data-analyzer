#!/usr/bin/env python3
"""Rebuild anonymous combat-time priors and whole-run validation folds.

Uses only the Python standard library. Runtime never reads the source logs.
"""

import argparse
import datetime as dt
import json
import math
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
BASE = [0.06, 0.085, 0.095, 0.102, 0.11, -0.03]
UPDATED_LOG_DATE = "2026-10-22"
DIM = len(BASE)


def features(phase, combat):
    band = min(4, int(phase * 5))
    return [float(i == band) for i in range(5)] + [
        math.log(max(30, combat) / 420)
    ]


def solve(matrix, rhs):
    """Pivoted Gaussian elimination; ridge precision makes the system full rank."""
    for col in range(len(rhs)):
        pivot = max(range(col, len(rhs)), key=lambda row: abs(matrix[row][col]))
        matrix[col], matrix[pivot] = matrix[pivot], matrix[col]
        rhs[col], rhs[pivot] = rhs[pivot], rhs[col]
        divisor = matrix[col][col]
        if not math.isfinite(divisor) or abs(divisor) < 1e-12:
            raise ValueError("Singular training system")
        for k in range(col, len(rhs)):
            matrix[col][k] /= divisor
        rhs[col] /= divisor
        for row in range(len(rhs)):
            if row == col:
                continue
            factor = matrix[row][col]
            for k in range(col, len(rhs)):
                matrix[row][k] -= factor * matrix[col][k]
            rhs[row] -= factor * rhs[col]
    return rhs


def fit(rows, prior, ridge):
    """Four IRLS passes prevent one unusual stage dominating a version prior."""
    coefficients = prior[:]
    for iteration in range(4):
        matrix = [
            [ridge[i] if i == j else 0.0 for j in range(DIM)]
            for i in range(DIM)
        ]
        rhs = [ridge[i] * prior[i] for i in range(DIM)]
        for row in rows:
            x = features(row["phase"], row["combat_seconds"])
            y = row["next_phase"] - row["phase"]
            residual = y - sum(a * b for a, b in zip(x, coefficients))
            weight = 1.0 if iteration == 0 else 1 / (1 + (residual / 0.03) ** 2)
            for i in range(DIM):
                rhs[i] += weight * x[i] * y
                for j in range(DIM):
                    matrix[i][j] += weight * x[i] * x[j]
        coefficients = solve(matrix, rhs)
        coefficients[5] = min(0, max(-0.2, coefficients[5]))
    return coefficients


def extract(directory):
    """Split on room changes and phase resets; keep EOF sessions too."""
    runs = []
    for path in sorted(directory.glob("output_log_*.txt")):
        stages = []

        def flush():
            if len(stages) < 3:
                return
            rows = []
            for first, second in zip(stages, stages[1:]):
                if first.get("end") is None:
                    continue
                combat = first["end"] - first["second"]
                cycle = second["second"] - first["second"]
                delta = second["phase"] - first["phase"]
                if (30 <= combat <= cycle <= 1800
                        and 0.025 <= delta <= 0.16 and second["phase"] < 1):
                    rows.append(dict(
                        phase=first["phase"], next_phase=second["phase"],
                        combat_seconds=combat, lobby_seconds=cycle - combat,
                    ))
            if len(rows) >= 2:
                log_date = path.name[len("output_log_"):len("output_log_") + 10]
                runs.append(dict(
                    id=f"{path.stem}:{len(runs)}",
                    era="updated" if log_date >= UPDATED_LOG_DATE else "historical",
                    starts_at_origin=stages[0]["phase"] <= 0.001,
                    reaches_final=stages[-1]["phase"] == 1,
                    stages=stages, rows=rows,
                ))

        for line in path.read_text(errors="replace").splitlines():
            if not re.match(r"^\d{4}\.\d\d\.\d\d \d\d:\d\d:\d\d", line):
                continue
            # Only differences are used; a fixed timezone avoids DST artefacts.
            second = int(dt.datetime.strptime(line[:19], "%Y.%m.%d %H:%M:%S")
                         .replace(tzinfo=dt.timezone.utc).timestamp())
            if "[Behaviour] OnLeftRoom" in line or "Entering Room:" in line:
                flush()
                stages = []
            match = re.search(r"now in stage: .*? on phase: ([0-9.]+)", line)
            if match:
                phase = float(match[1])
                if stages and phase < stages[-1]["phase"] - 0.01:
                    flush()
                    stages = []
                stages.append(dict(second=second, phase=phase))
            elif ("now in intermission" in line or "now in lobby" in line) and stages:
                stages[-1].setdefault("end", second)
        flush()
    return runs


def train(runs):
    historical = [row for run in runs if run["era"] == "historical" for row in run["rows"]]
    updated = [row for run in runs if run["era"] == "updated" for row in run["rows"]]
    old = fit(historical, BASE, [0.5] * 5 + [0.15])
    new = fit(updated, old, [0.8] * 5 + [0.15])
    def model(name, coefficients, rows):
        durations = [row["combat_seconds"] for row in rows] or [30, 1800]
        return dict(name=name, coefficients=coefficients,
                    combat_seconds_range=[min(durations), max(durations)])

    return [model("historical", old, historical),
            model("updated", new, updated or historical)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--extract", type=Path, help="Refresh the snapshot from this log directory")
    parser.add_argument("--check", action="store_true", help="Check generated artifacts without writing")
    args = parser.parse_args()
    if args.extract and args.check:
        parser.error("--extract and --check cannot be combined")

    def emit(path, value):
        text = json.dumps(value, indent=2) + "\n"
        if args.check:
            if path.read_text() != text:
                raise SystemExit(f"Stale generated model: {path}")
        else:
            path.write_text(text)

    corpus = ROOT / "resources/rules/step-training.json"
    if args.extract:
        runs = extract(args.extract)
        if not runs:
            raise SystemExit("No complete training transitions found")
        emit(corpus, dict(schema=1, runs=runs))
    else:
        runs = json.loads(corpus.read_text())["runs"]
    output = dict(
        schema=1,
        features=[f"phase_band_{i}" for i in range(5)] + ["log_combat_seconds_over_420"],
        models=train(runs), training_runs=len(runs),
        training_transitions=sum(len(run["rows"]) for run in runs),
    )
    emit(ROOT / "resources/rules/step-priors.json", output)
    print(json.dumps(output, indent=2))

    folds = []
    for run in runs:
        held = train([candidate for candidate in runs if candidate["id"] != run["id"]])
        coefficients = held[run["era"] == "updated"]["coefficients"]
        errors = [
            sum(a * b for a, b in zip(features(row["phase"], row["combat_seconds"]), coefficients))
            - (row["next_phase"] - row["phase"])
            for row in run["rows"]
        ]
        mae = sum(map(abs, errors)) / len(errors)
        folds.append(dict(run_id=run["id"], models=held, phase_mae=mae))
        print(run["id"], len(errors), "held-out phase MAE", round(mae, 5))
    emit(ROOT / "resources/rules/step-validation.json", dict(schema=1, folds=folds))


if __name__ == "__main__":
    main()
