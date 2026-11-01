#!/usr/bin/env python3
"""Draft and review eval seed items (eval/seed.jsonl, #22; decided in wi-100d6d).

    python3 scripts/seed.py save <transcript.jsonl> --source <session id> [--repo <owner/name>]
    python3 scripts/seed.py review
    python3 scripts/seed.py selftest

`save` drafts one item per end-of-turn assistant message in a Claude Code transcript
(~/.claude/projects/<project>/<session>.jsonl): `capture` when the message carries a `weigh-in`
block, `skip` otherwise, all `by: claude-draft`. It skips messages the seed already holds and
prints what it added. Read the drafts before committing: relabel prose calls the message made
without a block as `capture`, and redact client names, emails, tokens and personal data
(eval/README.md).

`review` walks the `claude-draft` items one at a time and records the label you give as
`by: ryan`.

Only the standard library is used, so any Python 3.9 or later runs it.
"""

from __future__ import annotations

import argparse
import datetime
import json
import re
import sys
import tempfile
from pathlib import Path

SEED = Path(__file__).resolve().parent.parent / "eval" / "seed.jsonl"
BLOCK = re.compile(r"```weigh-in\n(.*?)```", re.DOTALL)


def read_seed(path: Path) -> list[dict]:
    if not path.exists():
        return []
    with path.open(encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def dump(item: dict) -> str:
    return json.dumps(item, ensure_ascii=False) + "\n"


def write_seed(path: Path, items: list[dict], raw: list[str] | None = None) -> None:
    """Write the seed. Lines whose item is unchanged keep their original text (`raw`), so a
    review rewrites only the lines it labelled."""
    lines = []
    for k, item in enumerate(items):
        old = raw[k] if raw is not None and k < len(raw) else None
        lines.append(old if old is not None and json.loads(old) == item else dump(item))
    # Serialised before the file is opened, so an item that cannot be written leaves the seed
    # as it was. LF endings, as everywhere in the repository.
    text = "".join(lines)
    with path.open("w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def raw_lines(path: Path) -> list[str]:
    with path.open(encoding="utf-8", newline="") as f:
        return [line if line.endswith("\n") else line + "\n" for line in f if line.strip()]


def end_of_turn_messages(transcript: Path) -> list[str]:
    """The text of each assistant message that ended a turn, in order."""
    texts: dict[str, list[str]] = {}
    order: list[str] = []
    ended: set[str] = set()
    with transcript.open(encoding="utf-8") as f:
        for line in f:
            if not line.strip():
                continue
            entry = json.loads(line)
            message = entry.get("message")
            if entry.get("type") != "assistant" or not isinstance(message, dict):
                continue
            mid = message.get("id") or entry.get("uuid", "")
            if mid not in texts:
                texts[mid] = []
                order.append(mid)
            for block in message.get("content") or []:
                if isinstance(block, dict) and block.get("type") == "text":
                    texts[mid].append(block.get("text", ""))
            if message.get("stop_reason") == "end_turn":
                ended.add(mid)
    out = []
    for mid in order:
        text = "\n\n".join(t for t in texts[mid] if t).strip()
        if mid in ended and text:
            out.append(text)
    return out


def field(block: str, name: str) -> str:
    m = re.search(rf"^{name}:\s*(.+)$", block, re.MULTILINE)
    return m.group(1).strip() if m else ""


def draft(text: str) -> tuple[str, dict | None, str]:
    """A draft label, expectation and reason for one message."""
    blocks = BLOCK.findall(text)
    if not blocks:
        return "skip", None, "draft: no weigh-in block; relabel capture if it made a call in prose"
    first = blocks[0]
    defaults = field(first, "door") == "two-way" and field(first, "authority") == "judgment"
    status = "defaulted" if defaults else "raised"
    return "capture", {"status": status}, f"draft: {len(blocks)} weigh-in block(s)"


def next_session(items: list[dict]) -> int:
    numbers = [int(m.group(1)) for i in items if (m := re.fullmatch(r"s(\d+)", i.get("session", "")))]
    return max(numbers, default=0) + 1


def save(transcript: Path, source: str, repo: str, seed: Path, today: str) -> list[dict]:
    items = read_seed(seed)
    known = {i.get("text") for i in items}
    fresh = [t for t in end_of_turn_messages(transcript) if t not in known]
    if not fresh:
        return []
    n = next_session(items)
    added = []
    for k, text in enumerate(fresh, start=1):
        label, expect, why = draft(text)
        added.append(
            {
                "id": f"rs{n}-{k:02}",
                "session": f"s{n}",
                "source": source,
                "repo": repo,
                "origin": "real-message",
                "channel": "marker",
                "text": text,
                "label": label,
                "expect": expect,
                "why": why,
                "by": "claude-draft",
                "on": today,
            }
        )
    text = "".join(dump(i) for i in added)
    with seed.open("a", encoding="utf-8", newline="\n") as f:
        f.write(text)
    return added


def review(seed: Path, today: str, ask=input, say=print) -> int:
    raw = raw_lines(seed)
    items = [json.loads(line) for line in raw]
    done = 0
    for item in items:
        if item.get("by") != "claude-draft":
            continue
        say(f"\n--- {item['id']} ({item['label']}, {item.get('why', '')})\n{item['text']}\n")
        answer = ask("[c]apture [r]aised / [d]efaulted, [s]kip, [k]eep draft, [q]uit: ").strip().lower()
        if answer == "q":
            break
        if answer in ("c", "r"):
            item["label"], item["expect"] = "capture", {"status": "raised"}
        elif answer == "d":
            item["label"], item["expect"] = "capture", {"status": "defaulted"}
        elif answer == "s":
            item["label"], item["expect"] = "skip", None
        else:
            continue
        item["by"], item["on"] = "ryan", today
        done += 1
    write_seed(seed, items, raw)
    return done


def selftest() -> None:
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        seed = root / "seed.jsonl"
        seed.write_text('{"id": "rs3-01", "session": "s3", "text": "old \\u2014", "by": "ryan"}\n',
                        encoding="utf-8")
        lines = [
            {"type": "user", "message": {"role": "user", "content": "hi"}},
            {"type": "assistant", "message": {"id": "m1", "stop_reason": "tool_use",
                                              "content": [{"type": "text", "text": "working"}]}},
            {"type": "assistant", "message": {"id": "m2", "content": [{"type": "thinking"}]}},
            {"type": "assistant", "message": {"id": "m2", "stop_reason": "end_turn", "content": [
                {"type": "text", "text": "Done.\n\n```weigh-in\nkind: call\ndoor: two-way\n"
                                         "authority: judgment\nq: Keep it?\n```"}]}},
            {"type": "assistant", "message": {"id": "m3", "stop_reason": "end_turn",
                                              "content": [{"type": "text", "text": "old \u2014"}]}},
            {"type": "assistant", "message": {"id": "m4", "stop_reason": "end_turn",
                                              "content": [{"type": "text", "text": "Plain é."}]}},
        ]
        transcript = root / "t.jsonl"
        transcript.write_text("".join(json.dumps(x) + "\n" for x in lines), encoding="utf-8")

        added = save(transcript, "session_x", "o/r", seed, "2026-11-01")
        assert [a["id"] for a in added] == ["rs4-01", "rs4-02"], added
        assert added[0]["label"] == "capture" and added[0]["expect"] == {"status": "defaulted"}, added
        assert added[1]["label"] == "skip" and added[1]["expect"] is None, added
        assert "Plain é." in seed.read_text(encoding="utf-8"), "non-ASCII is written as is"
        assert save(transcript, "session_x", "o/r", seed, "2026-11-01") == [], "saved twice"

        answers = iter(["s", "q"])
        n = review(seed, "2026-11-02", ask=lambda _: next(answers), say=lambda *_: None)
        after = read_seed(seed)
        assert n == 1, n
        assert after[1]["by"] == "ryan" and after[1]["label"] == "skip", after[1]
        assert after[2]["by"] == "claude-draft", after[2]
        assert raw_lines(seed)[0].startswith('{"id": "rs3-01"') and "\\u2014" in raw_lines(seed)[0], \
            "an untouched line keeps its original text"
    print("selftest: ok")


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("save", help="draft items from a session transcript")
    s.add_argument("transcript", type=Path)
    s.add_argument("--source", required=True, help="the session id, as session_…")
    s.add_argument("--repo", default="Wet-Ink-Corporation/weigh-in")
    s.add_argument("--seed", type=Path, default=SEED)
    r = sub.add_parser("review", help="label the claude-draft items")
    r.add_argument("--seed", type=Path, default=SEED)
    sub.add_parser("selftest", help="check this script on a scratch seed")
    a = p.parse_args(argv)
    today = datetime.date.today().isoformat()
    if a.cmd == "save":
        added = save(a.transcript, a.source, a.repo, a.seed, today)
        for item in added:
            print(f"{item['id']}  {item['label']:7}  {item['text'][:70]!r}")
        print(f"added {len(added)} item(s) to {a.seed}")
    elif a.cmd == "review":
        print(f"reviewed {review(a.seed, today)} item(s)")
    else:
        selftest()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
