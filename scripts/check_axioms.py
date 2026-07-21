#!/usr/bin/env python3
"""Fail if Lean axioms drift from docs/axioms.md inventory."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEAN_ROOT = ROOT / "src" / "lean"
INVENTORY = ROOT / "docs" / "axioms.md"

AXIOM_DECL = re.compile(r"(?m)^\s*axiom\s+([A-Za-z0-9_']+)")
INVENTORY_ROW = re.compile(
    r"\|\s*`[^`]+`\s*\|\s*`([A-Za-z0-9_']+)`\s*\|"
)


def collect_lean_axioms() -> dict[str, list[Path]]:
    found: dict[str, list[Path]] = {}
    for path in LEAN_ROOT.rglob("*.lean"):
        text = path.read_text(encoding="utf-8")
        for match in AXIOM_DECL.finditer(text):
            name = match.group(1)
            found.setdefault(name, []).append(path.relative_to(ROOT))
    return found


def collect_inventory() -> set[str]:
    text = INVENTORY.read_text(encoding="utf-8")
    # Only the "Allowed axioms" table counts; later theorem tables must not match.
    allowed = re.search(
        r"## Allowed axioms \(inventory\)(.*?)(?:\n## |\Z)",
        text,
        flags=re.DOTALL,
    )
    if not allowed:
        raise SystemExit("docs/axioms.md missing '## Allowed axioms (inventory)' section")
    section = allowed.group(1)
    names = set(INVENTORY_ROW.findall(section))
    count_match = re.search(r"\*\*Count:\*\*\s*(\d+)\s*axioms", section)
    if not count_match:
        raise SystemExit(
            "docs/axioms.md missing '**Count:** N axioms' in Allowed axioms section"
        )
    declared_count = int(count_match.group(1))
    if declared_count != len(names):
        raise SystemExit(
            f"docs/axioms.md Count={declared_count} but table lists {len(names)} axioms: "
            f"{sorted(names)}"
        )
    return names


def main() -> int:
    lean = collect_lean_axioms()
    inventory = collect_inventory()
    lean_names = set(lean)

    errors: list[str] = []
    missing_docs = sorted(lean_names - inventory)
    extra_docs = sorted(inventory - lean_names)
    if missing_docs:
        errors.append(f"axioms in Lean but not in docs/axioms.md: {missing_docs}")
    if extra_docs:
        errors.append(f"axioms in docs/axioms.md but not in Lean: {extra_docs}")
    if len(lean_names) != len(inventory):
        errors.append(
            f"axiom count mismatch: lean={len(lean_names)} inventory={len(inventory)}"
        )

    if errors:
        print("Axiom inventory check FAILED:")
        for err in errors:
            print(f"  - {err}")
        print("\nLean axioms:")
        for name in sorted(lean):
            locs = ", ".join(str(p) for p in lean[name])
            print(f"  {name}: {locs}")
        return 1

    print(f"Axiom inventory OK ({len(inventory)} axioms; no formal-completeness claim).")
    for name in sorted(inventory):
        print(f"  - {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
