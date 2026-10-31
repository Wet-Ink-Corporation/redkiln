"""Validate the suite label manifest, .github/labels.yml (Backlog call A, wi-cef669).

Run: python3 tools/labels/check.py [path]. Needs PyYAML. Exits 0 when valid, 1 with one line per
problem otherwise. Checks: the version, each family's prefix and rule, that every label belongs to
exactly one non-reserved family, unique names, 6-digit hex colours, descriptions within GitHub's
100 characters, and that every exactly-one, at-least-one or exactly-one-while-open family has
labels.
"""

import re
import sys
from pathlib import Path

import yaml

RULES = {"exactly-one-while-open", "exactly-one", "at-least-one", "zero-or-more", "at-most-one", "reserved"}
NEEDS_LABELS = {"exactly-one-while-open", "exactly-one", "at-least-one"}
COLOR = re.compile(r"[0-9a-f]{6}")


def problems(manifest: dict) -> list[str]:
    found = []
    if manifest.get("version") != 1:
        found.append("version: expected 1")
    families = manifest.get("families") or []
    prefixes = {}
    for family in families:
        prefix, rule = family.get("prefix"), family.get("rule")
        if not isinstance(prefix, str) or not prefix.endswith(("/", ":")):
            found.append(f"family {prefix!r}: a prefix ends with / or :")
            continue
        if rule not in RULES:
            found.append(f"family {prefix}: unknown rule {rule!r}")
        if rule == "reserved" and not family.get("owner"):
            found.append(f"family {prefix}: a reserved family names its owner")
        if prefix in prefixes:
            found.append(f"family {prefix}: declared twice")
        prefixes[prefix] = rule
    seen = set()
    counts = dict.fromkeys(prefixes, 0)
    for label in manifest.get("labels") or []:
        name = label.get("name")
        if not isinstance(name, str) or not name:
            found.append(f"label {label!r}: no name")
            continue
        if name in seen:
            found.append(f"label {name}: declared twice")
        seen.add(name)
        owners = [p for p in prefixes if name.startswith(p) and len(name) > len(p)]
        if len(owners) != 1:
            found.append(f"label {name}: belongs to {len(owners)} families, expected 1")
        elif prefixes[owners[0]] == "reserved":
            found.append(f"label {name}: {owners[0]} is reserved for its owner")
        else:
            counts[owners[0]] += 1
        if not COLOR.fullmatch(str(label.get("color", ""))):
            found.append(f"label {name}: colour is 6 lower-case hex digits")
        description = label.get("description", "")
        if not isinstance(description, str) or not description or len(description) > 100:
            found.append(f"label {name}: description is 1 to 100 characters")
    for prefix, count in counts.items():
        if prefixes[prefix] in NEEDS_LABELS and count == 0:
            found.append(f"family {prefix}: rule {prefixes[prefix]} needs at least one label")
    return found


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else ".github/labels.yml")
    found = problems(yaml.safe_load(path.read_text(encoding="utf-8")) or {})
    for problem in found:
        print(f"{path}: {problem}")
    if not found:
        print(f"{path}: ok")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
