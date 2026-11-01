import json, random
s = json.load(open("state.json"))
perms = [[2,0,1],[1,2,0],[2,1,0]]  # three distinct orders, none equal to the original
labels = "PQR"
for j, perm in enumerate(perms, 1):
    out = ["# Decision cards\n",
           "Constraints in force (settled decisions, never re-open): the mechanism is a Redkiln-native decision item type built on a portable Claude Code plugin core; decided records land in the same .kb layout in every repo; judge models are Anthropic models only; the decider's agent work is about half interactive, half unattended. The decider is a very senior solo architect (Wet Ink Corporation, and a consulting firm, IQBG, with federal/state/local and commercial clients). He prefers long-term value and quality over shortcuts and dislikes recommendations that paper over problems or defer what can be settled now.\n"]
    for it in s["items"]:
        opts = [it["options"][i] for i in perm]
        out.append(f"\n## {it['id']}: {it['question']}\n")
        out.append(f"Why it matters: {it['why']}\n")
        out.append(f"Door: {it['door']} ({it['door_note']}). Blast radius: {it['blast']}. Resolve by: {it['resolve_by']}.\n")
        out.append("Evidence:\n" + "\n".join(f"- [{e['src']}] {e['text']}" for e in it["evidence"]) + "\n")
        if it.get("excerpt"):
            ex = it["excerpt"]
            out.append(f"Excerpt {ex['path']}:{ex['start']}:\n```\n" + "\n".join(ex["lines"]) + "\n```\n")
        out.append("Options:")
        for lab, o in zip(labels, opts):
            extra = ""
            if o.get("fails_must"): extra += f" FAILS MUST: {o['fails_must']}."
            if o.get("evasion"): extra += f" Evasion flag: {o['evasion']}."
            out.append(f"- **{lab}. {o['title']}** — {o['summary']} Holds if {o['holds_if']}. If wrong: {o['if_wrong']}.{extra}")
        out.append("")
    open(f"judge{j}.md","w").write("\n".join(out))
    # key to map labels back
    json.dump({it["id"]: {labels[k]: it["options"][perm[k]]["id"] for k in range(3)} for it in s["items"]}, open(f"judge{j}.key.json","w"))
print(open("judge1.md").read()[:3000])
