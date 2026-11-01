# Presenting codebase context for fast, well-informed HITL decisions (toolkit, evidence, token cost)

Scope note: research current to 26 September 2026. Evidence labels used throughout: **[Measured]** = controlled experiment or mining study with outcome data; **[Perceived]** = survey or interview, self-reported; **[Vendor]** = product docs or blog; **[Issue]** = public issue tracker report. The principle "every claim verifiable in seconds or left out" is taken as given and not re-researched here.

## Q1. What does empirical research say reviewers and deciders need to understand code changes and design questions quickly?

### Takeaway
Across 13 years of studies, the bottleneck is *understanding*, not finding defects. Reviewers most often need (1) the rationale ("what instigated the change"), (2) whether an alternative solution would be better, (3) confirmation that they understood it correctly, and (4) code context such as invocation paths and blast radius. Small, decomposed, grouped units reduce errors. For agent PRs, rejection comes mostly from misalignment with the project and from reviewer abandonment, not from bad code as such. Richer AI explanations raise reviewers' trust, but they do not save time.

### Cited Findings
**Classic modern-code-review evidence**
- [Measured and Perceived] Bacchelli & Bird (ICSE 2013) found understanding to be the main challenge. Developer quotes: "the most difficult thing when doing a code review is understanding the reason of the change" and "the biggest information need in code review: what instigated the change". Of 570 comments, 29% were code improvements and only 14% were defects. 91% of respondents (798/873) said unfamiliar code takes substantially more time to review. 20–40% of reviews still needed an in-person discussion. Existing tools offered little comprehension support beyond the diff. — [Bacchelli & Bird 2013](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/ICSE202013-codereview.pdf)
- [Measured] At Google, the median change touches 24 lines. Over 35% of changes modify one file and about 90% modify fewer than 10 files. The median reviewer count is 1. Median initial feedback comes in under 1 hour for small changes and about 5 hours for large ones. "Context (missing rationale)" is one of five breakdown categories. Developers use the review tool "long after those changes have been approved" to examine history and past decisions. 97% are satisfied with Critique. — [Sadowski et al., ICSE-SEIP 2018](https://sback.it/publications/icse2018seip.pdf)
- [Measured] A study of 900 review threads in OpenStack, Android and Qt found seven information needs, from most to least frequent: N1 suitability of an alternative solution (significantly the most frequent), N2 correct understanding, N3 rationale, N4 code context (implementation context and invocation paths), N5 necessity, N6 specialized expertise, N7 splittable. Developers rated N1 and N2 the most critical. — [Pascarella et al., ACM CSCW 2018](https://fpalomba.github.io/pdf/Conferencs/C36.pdf)
- [Measured] In an experiment with 50 mostly professional developers, working-memory capacity was moderately associated with finding *delocalized* defects (defects that span several code locations). Review effectiveness was significantly higher for small changes. The effect of the order in which change parts were presented was inconclusive. — [Baum, Schneider & Bacchelli, EMSE 2019](https://link.springer.com/article/10.1007/s10664-018-9676-8)
- [Perceived; 12 interviews, 201 survey respondents, 292 logged sessions] Six ordering principles for change parts: (1) group related parts as closely as possible; (2) provide information before it is needed; (3) prefer grouping over direction when the two conflict; (4) chunk closely related parts; (5) the closest distance is "visible on the screen at the same time"; (6) use ordering rules the reviewer can understand, and make the grouping explicit. Relation types: data flow, call flow (bottom-up preferred), class hierarchy, declaration and use, similarity, logical dependency, development flow. — [Baum et al., ICSME 2017](https://sback.it/publications/icsme2017.pdf)
- [Measured; 28 developers] Splitting a tangled PR (refactor plus feature) into separate PRs produced significantly fewer false-positive findings (p=0.03, medium effect) and more context-seeking behaviour. Defects found, review time and understanding of the rationale did not differ significantly. — [Di Biase et al., PeerJ CS 2019](https://pmc.ncbi.nlm.nih.gov/articles/PMC7924728/)
- [Measured; 23,241 PRs] 44.6% of reviews did not follow GitHub's alphabetical file order. Of PRs containing both test and production files, 29% were reviewed test-first. Reviews in a non-alphabetical order covered more files (42.1% vs 39.0%, p<0.001). The authors recommend showing diff size in the file tree, file-level summaries, and entry-point hints instead of a prescribed order. — [Bouraffa et al., EASE 2025](https://azaidman.github.io/publications/bouraffaEASE2025.pdf)
- [Observational; 10 reviewers, 25 real reviews] Reviewers start with a context-building phase before inspecting code, and they use opportunistic rather than systematic strategies. They contrast their mental picture of the "expected/ideal solution" with the actual implementation. — [Gonçalves et al., arXiv 2025](https://pith.science/paper/2503.21455)

**AI-generated and agentic PRs (2024–2026)**
- [Measured; 18,256 Copilot-for-PRs PRs vs 54,188 controls] AI-generated PR descriptions were associated with review time 19.3 hours shorter and a 1.57× higher chance of merge (84% vs 71%). Developers deleted (22.9%), refined (20.8%) or excluded (17.4%) generated content, the poem especially. They added templates (22.8%) and relevant links (22.7%). — [Xiao et al., FSE 2024](https://tao-xiao.github.io/files/Copilot4PR_FSE_2024.pdf)
- [Measured; 567 Claude Code PRs in 157 projects] 83.8% were merged, and 54.9% of those were merged without modification. Human revisions concentrated on bug fixes, documentation, and adherence to project conventions. — [Watanabe et al., arXiv 2509.14745 v3, Feb 2026](https://arxiv.org/abs/2509.14745v3)
- [Measured; 33,596 agent PRs] Merge rates: Codex 82.6%, Cursor 65.2%, Claude Code 59.0%, Devin 53.8%, Copilot 43.0%. Among 600 rejected PRs, the causes were reviewer abandonment with no meaningful interaction (38%), duplicates (23%), CI/test failures (17%), unwanted features (4%), incorrect or incomplete work (5%), and ignoring reviewer instructions (1%). Larger diffs and more files lowered merge odds, and each failed CI check reduced the odds by about 15%. — [Ehsani et al., arXiv 2601.15195, Jan 2026](https://arxiv.org/html/2601.15195)
- [Measured; 9,428 agentic PRs in the AIDev Python subset] Claude reached an 84.3% merge rate, Codex 73.5% and Devin 43.0%. Complex features "requiring extensive contextual understanding" merged least often (≤0.60). — [Mazloomzadeh et al., arXiv 2607.21832, Jul 2026](https://arxiv.org/html/2607.21832v1). **Conflict:** Claude Code's merge rate is 59% in Ehsani et al. and 84% in Watanabe et al. and Mazloomzadeh et al. The datasets and filters differ, so no single number should be quoted.
- [Perceived; participatory design with 17 and 7 developers, 43-person survey] Reviewers of LLM-generated multi-file changes cannot scale their effort to risk when the AI shows uniform confidence. They asked for line- and file-level risk indicators (based on historical change frequency), rationale walk-throughs, decomposition into minimal independently applicable "chunks", traffic-light verdicts, and architectural diagrams. The proposed workflow has three levels: overview, then file, then snippet. 63% expected lower review effort. — [Heander et al., arXiv 2606.01969, Jun 2026](https://arxiv.org/html/2606.01969v1)
- [Measured; within-subjects, 34 programmers, 9 PRs] Detailed inline explanations with code highlighting gave the highest trust (3.99/5), but agreement with the AI was lower than with review comments alone (89.2%). A binary accept/reject recommendation alone gave the lowest agreement (66.7%). Time did not differ significantly between conditions. — [Gao et al., Jul 2026 (via Pith summary)](https://pith.science/paper/2607.24601)

### Inferences
- **What a decision card should contain, ranked by evidence strength.**
  1. **Rationale / trigger**: why the question arose (Bacchelli's #1 need; Sadowski's "context" breakdown).
  2. **Options with trade-offs**: "is there a better alternative" is the single most frequent reviewer need (Pascarella N1). Options are therefore the core of the card, not an optional extra.
  3. **"Did I understand correctly" check**: a one-line statement of expected behaviour for each option, which lets the human run the expected-vs-actual comparison Gonçalves describes.
  4. **Code context / blast radius**: call sites and dependents (Pascarella N4). Delocalized defects are the ones that overload working memory (Baum 2019).
  5. **Prior decisions / history**: Google developers mine past decisions in the review tool. Link ADRs or earlier ledger entries.
  6. **Tests / CI state**: failed CI and misalignment dominate agent-PR rejection. 29% of reviewers go test-first.
  7. **Necessity**: "do we need this at all" (Pascarella N5). Always include a "do nothing / defer" option.
  8. **Ownership**: matters in teams (Bacchelli), less so for a solo founder. "Last touched by / when" is enough.
- **Runtime behaviour** (logs, traces, benchmark numbers) appears in none of these studies as a top need. Include it only when the question is about performance or behaviour; see Gaps.
- **Presentation rules drawn from Baum's principles:** one decision per card (decomposition cuts false positives). Keep related evidence on one screen (principle 5). State the grouping explicitly (principle 6). Give the definition or context before the dependent detail (principle 2). Offer entry points, not a forced order (Bouraffa).
- **Do not replace evidence with a verdict.** A bare recommendation performed worst on agreement (Gao 2026). Rich explanations did not cost time, but they raised trust without raising agreement. The recommendation should therefore arrive *with* two or three verifiable anchors, not with a long rationale.
- **Risk signal first.** Uniform confidence prevents effort allocation (Heander 2026). Each card should carry a reversibility/risk tag so the human can approve low-risk items in seconds and spend minutes on the rest.

### Gaps
- No controlled study was found that measures how *decision* time (as opposed to review time) changes with specific context elements such as blast radius or history.
- Runtime/trace context: no 2024–2026 study found that isolates its value for review decisions.
- Could not read "Help Me to Understand this Commit! A Vision for Contextualized Code Reviews" ([arXiv 2402.09528](https://arxiv.org/html/2402.09528)): arXiv returned HTTP 429 (rate limited) and the page was not retried.
- The ACM page for Bouraffa et al. returned 403. The authors' PDF was used instead.

## Q2. How do AI code-review and agent tools present context to humans in 2026, and which elements do users praise or ignore?

### Takeaway
Vendors converge on the same components: a short summary, a severity-ranked list of findings anchored at file:line, suggested fixes, a confidence or effort score, optional diagrams, and deep links to fix. Measured data says what people act on is **hunk-anchored, concise, code-heavy, on-demand** comments. Long walkthrough extras (poems, effort scores, related PRs, sequence diagrams, suggested labels) are the parts teams switch off.

### Cited Findings
**How tools present context [Vendor]**
- **CodeRabbit walkthrough.** Configurable sections, all on by default: `changed_files_summary`, `sequence_diagrams` (Mermaid), `estimate_code_review_effort` (a 1–5 score from file count, nature of change and logic), `related_issues`, `assess_linked_issues`, `related_prs`, `suggested_labels`, `suggested_reviewers`, `poem`, `in_progress_fortune`. The walkthrough is collapsed by default. — [CodeRabbit docs](https://docs.coderabbit.ai/pr-reviews/walkthroughs)
- **CodeRabbit "Change Stack" (13 May 2026).** Groups a PR into "independent change cohorts" with "ordered layers that reflect the natural reading order". Three panels: navigation, diff, and per-range context. Diagrams (sequence, state machine, ER) appear only where they help: "not every layer gets one". J/K keyboard navigation. No user-study data. — [CodeRabbit blog](https://www.coderabbit.ai/blog/introducing-atlas-the-first-ai-native-code-review-interface)
- **Greptile.** Plain-language summary. Confidence score 0–5 (5 = production ready; 4 = minor polish; 3 = issues to address; 2 = significant bugs; 0–1 = critical). File-by-file table of changes and issues. The diagram type is chosen by change type: sequence for API flows, ER for schema, class for hierarchy, flow for business logic. "Minimal changes generate no diagram." Inline P0/P1/P2 badges with applicable suggestions. — [Greptile docs](https://www.greptile.com/docs/code-review/first-pr-review)
- **Devin Review.** "Smart diff organization" groups related edits instead of listing files alphabetically. Detects copied and moved code so it shows as a move, not as a delete plus insert. Opens with an overview and key-change bullets. Severe and non-severe bug tiers, plus security Critical/Warning with CWE classification. "Codebase-aware chat" for questions. — [Devin docs](https://docs.devin.ai/work-with-devin/devin-review)
- **Claude Code Code Review (research preview).** Parallel agents, then a *verification step* that filters false positives against actual code behaviour. Severity markers: 🔴 Important, 🟡 Nit, 🟣 Pre-existing. Each finding has a collapsible "extended reasoning" section. The check run holds a summary table (Severity | File:Line | Issue). The docs suggest REVIEW.md rules such as "behavior claims need a `file:line` citation in the source, not an inference from naming", "report at most five nits", and "open with a one-line tally… lead with 'no factual issues'". Averages $15–25 and about 20 minutes per review. In host apps, local `/code-review` reports through a `ReportFindings` list (file location, one-sentence summary, category tag). Later passes mark each finding fixed, skipped, or no change needed. — [Claude Code docs: Code Review](https://code.claude.com/docs/en/code-review)
- **Cursor Bugbot.** Each finding has a title, explanation, file/line, and a blocking or non-blocking flag. Severity is high or medium. **"Fix in Cursor" and "Fix in Web" deep links**, optional Autofix via a cloud agent, and per-rule acceptance-rate analytics. — [Cursor Bugbot docs](https://cursor.com/docs/bugbot)
- **GitHub Copilot code review.** An overview comment carries an *approval assessment* and the review effort level used. Suggested changes apply in a couple of clicks. Some file types, such as lockfiles and logs, are excluded. — [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/code-review)
- **Qodo.** Each finding gives "what needs attention, why it matters, and how to fix it, ranked by severity". A PR summary comes first, with configurable verbosity and visibility. — [Qodo docs](https://docs.qodo.ai/code-review)
- **Graphite Agent (Oct 2025).** Positioned as review plus chat plus applying fixes in one place. The launch post contains no quantitative metrics. — [Graphite blog](https://graphite.com/blog/introducing-graphite-agent-and-pricing)

**What users act on or ignore [Measured, plus one practitioner config]**
- [Measured; >22,000 comments from 16 AI-review GitHub Actions in 178 repos] Hunk-level comments were addressed 6.5–19.2% of the time; file-level comments 0.9–4.2%. Human review comments were addressed about 60% of the time. Comments that were concise, or mostly code (code ratio >0.5, especially multi-line code blocks), were addressed more often. Manually triggered comments beat automatic ones (12.8% vs 6.8% for one action). — [Sun, …, Treude, 2026](https://ctreude.ca/pdf/does-ai-code-review-lead-2026.pdf)
- [Measured; AIDev corpus] PRs reviewed only by code-review agents merged 45.2% of the time, against 68.4% for PRs reviewed only by humans. 60.2% of closed agent-only PRs fell in the lowest signal band (0–30% useful comments). 12 of 13 agents averaged below 60% signal. — [Banik et al., Apr 2026](https://pith.science/paper/2604.03196)
- [Measured, industrial; 4,335 PRs with Qodo PR-Agent] 73.8% of automated comments were marked resolved, yet mean PR closure time *rose* from 5h52m to 8h20m. Complaints: "faulty reviews, unnecessary corrections, and overly frequent or irrelevant comments". — [Cihan et al., ICSE-SEIP 2025](https://conf.researchr.org/details/icse-2025/icse-2025-software-engineering-in-practice/8/Automated-Code-Review-In-Practice)
- [Practitioner config] PostHog switched off nine CodeRabbit walkthrough sections: changed-files summary, sequence diagrams, review-effort estimate, linked-issue assessment, related issues and PRs, suggested labels and reviewers, and fortune messages. The reason: "A reviewer … meets two CodeRabbit comments and has to read both to find the review. Only one holds findings." They kept the short summary with priority and merge risk, plus the pre-merge checks. — [PostHog PR #100461](https://github.com/PostHog/posthog/pull/100461)
- [Measured] Developers excluded whole generated sections, the poem most of all, from Copilot PR descriptions, and added links. — [Xiao et al., FSE 2024](https://tao-xiao.github.io/files/Copilot4PR_FSE_2024.pdf)
- [Perceived] A graph visualisation of merge requests (ReviewVis) was rated clear by 78% and useful by 89% (n=9 in-company). 80% of 31 surveyed said it would help understanding. Actual review performance was not measured. — [Fregnan et al., JSS 2023](https://www.sciencedirect.com/science/article/pii/S0164121222001820)

### Inferences
- **Worth copying for the ledger:**
  - a severity or risk marker plus a one-line tally at the top (Claude Code Review, Greptile);
  - findings anchored at file:line, with a collapsible "why / how verified" section (Claude Code Review);
  - a what / why / how-to-fix triad (Qodo);
  - a one-click action link (Bugbot "Fix in Cursor");
  - explicit grouping into cohorts or chunks (Change Stack, Devin);
  - diagrams only when the change type calls for one (Greptile, Change Stack);
  - a status lifecycle per item (fixed / skipped / no change needed), which maps directly to open / decided / deferred.
- **Leave out by default:** poems and fortunes; numeric "effort to review" scores; related-PR and label suggestions; file summaries that restate the diff. Teams remove these (PostHog, Xiao) and nothing shows they help.
- The strongest measured lever is **anchoring at hunk or line level, concise wording, and a high share of code**. A decision card should therefore show a small code excerpt at a precise location, not prose about a file.
- On-demand beats push (manual 12.8% vs automatic 6.8%). Let the human open a card's detail rather than dumping all cards into the conversation.
- Cihan's result shows that more comments can slow closure even when most get "resolved". Cap the number of open decisions surfaced at once, the way REVIEW.md caps nits.

### Gaps
- No independent measurement was found for Greptile's confidence score, CodeRabbit's effort score, or Copilot's approval assessment, either for calibration or for effect on decision time.
- No public data on Change Stack, Devin Review or Graphite Agent usability. Amp's review UI was not researched, for lack of tool budget.
- No HN or Reddit thread with quantified sentiment was found. The PostHog config is the most concrete practitioner signal located.

## Q3. How can context be extracted token-efficiently, how much snippet context is useful, and what should a script compute versus the model read?

### Takeaway
Graph-ranked symbol maps (Aider), LSP queries (definitions, references, call hierarchy), `git log -L`, dependency-graph CLIs with "affected" filters, and `--numstat` summaries can all run deterministically and emit compact, structured output. The model should read only the ranked, trimmed results. Evidence on snippet size is thin. Unified-diff convention is 3 lines of context. SWE-agent found 100-line windows best *for models*. Baum's "visible on one screen" principle is the best human-side bound.

### Cited Findings
- [Vendor/OSS] Aider's repo map lists the most important classes and functions with their signatures, ranked on a file-dependency graph, within a token budget: `--map-tokens` defaults to 1,000 and "expands… significantly" when no files are in the chat. — [Aider docs](https://aider.chat/docs/repomap.html); design background: [Aider blog 2023](https://aider.chat/2023/10/22/repomap.html)
- [Measured, for agents] SWE-agent's interface design found the file viewer "works best when displaying just 100 lines in each turn". For search it lists only the files with matches, because "showing the model more context about each match proved to be too confusing". — [SWE-agent ACI docs](https://swe-agent.com/0.7/background/aci/)
- [Vendor] The Claude Code LSP tool supports jump to definition, find all references, type info at a position, file symbols, workspace symbol search, find implementations, and call hierarchies. It needs a code-intelligence plugin plus a locally installed server binary. It is *inactive in cloud sessions*. It reports type errors automatically after each edit. — [Claude Code tools reference](https://code.claude.com/docs/en/tools-reference). Community LSP plugin marketplaces cover C# and TypeScript. — [boostvolt/claude-code-lsps](https://github.com/boostvolt/claude-code-lsps)
- [Docs] `git log -L <start>,<end>:<file>` and `-L :<funcname>:<file>` trace a line range or function through history, showing only the relevant patch per commit. Limits: one file only, no pathspec, no `--stat` or `--numstat`. — [git-log docs](https://git-scm.com/docs/git-log)
- [Docs] `git diff -U<n>` defaults to **3** lines of context. `-W/--function-context` shows the whole enclosing function. `--numstat` gives machine-friendly `<added> <deleted> <path>` lines. — [git-diff docs](https://git-scm.com/docs/git-diff)
- [Docs] dependency-cruiser (JS/TS) options:
  - `--affected <rev>`: "only include modules changed since the revision + all modules that can reach them";
  - `--reaches <pattern>`: transitive dependents;
  - `--focus` with `--focus-depth`: neighbours;
  - `--highlight "$(watskeburt main)"`: mark changed modules;
  - `-T mermaid`: minified by default "to circumvent mermaid's input size limits";
  - `-T text`: grep-friendly output;
  - `-T markdown`: output for PR summaries.
  — [dependency-cruiser CLI docs](https://github.com/sverweij/dependency-cruiser/blob/main/doc/cli.md). A PR workflow posts the Mermaid graph with changed files highlighted. Mermaid's default input limit is 50,000 characters. — [dev.to, Jan 2025](https://dev.to/mh4gf/visualize-typescript-dependencies-of-changed-files-in-a-pull-request-using-127j)
- [OSS claim] code-review-graph builds a tree-sitter knowledge graph for blast-radius analysis over files, functions and tests. It supports C# and VB.NET and exposes MCP and CLI. It claims a median ~63× (range 35–358×) token reduction against a *whole-corpus* baseline, which it admits is "an upper bound no real agent pays". — [code-review-graph README](https://github.com/tirth8205/code-review-graph)
- [Vendor] Anthropic's token-counting endpoint is free (rate-limited), and its counts are estimates. Use it to calibrate the card budgets below. — [Claude token counting docs](https://platform.claude.com/docs/en/build-with-claude/token-counting)
- [Measured] Working-memory limits matter mainly for delocalized issues. — [Baum 2019](https://link.springer.com/article/10.1007/s10664-018-9676-8). The ideal is "visible on the screen at the same time". — [Baum 2017](https://sback.it/publications/icsme2017.pdf)

### Inferences
- **What a script computes, deterministically, at zero model tokens:**
  - changed files with `--numstat`;
  - symbol outline of the touched files (tree-sitter or ctags);
  - references and callers of the symbol in question (LSP find-references or call hierarchy in local sessions; ripgrep for the identifier as a fallback in cloud sessions, where LSP is inactive);
  - dependents (dependency-cruiser `--affected` or `--reaches` for TypeScript; project references plus LSP references for .NET);
  - history of the exact range (`git log -L`, reduced by the script to "N commits; last <sha> <date> <subject>");
  - test files that reference the symbol;
  - CI or test status;
  - existing ADR or ledger entries matched by path or symbol;
  - SHA-pinned permalinks and editor URIs.
- **What the model does:** frame the question, write the 2–4 options, pick the 1–3 anchors worth showing, and write one-line consequences per option. The model reads only the script's JSON summary, never raw `git log -L` output or full reference lists.
- **Snippet sizing.** Keep each excerpt to about 3–15 lines: the changed or relevant lines plus about 3 lines on each side, following the diff convention. Expand to the whole function (`-W`) only when it fits on one screen, roughly ≤40 lines. This is an inference from the git default and Baum's one-screen principle. The 100-line figure is a *model*-reading optimum (SWE-agent) and should not be used for human-facing cards.
- **Rough token estimates** (my estimates, to be verified with the count_tokens endpoint):
  - a file:line anchor with a one-line claim: ~15–30 tokens;
  - a 10-line C# or TypeScript excerpt: ~100–180 tokens;
  - a 5-row affected-files table (path | +/- | role): ~80–150 tokens;
  - an 8–12-node Mermaid flowchart: ~120–250 tokens;
  - a raw `git log -L` on an active function: easily 2k–20k tokens (so summarise it by script);
  - a full Aider-style map: about 1k tokens by default.
- **Suggested budget.** About 150–300 tokens per card in the conversation (question, options, recommendation, 2–3 anchors). The full evidence pack goes to a file or HTML page that the human opens, and the model never re-reads it.

### Gaps
- No human-subject study was found that measures the best number of lines per code excerpt in a review or decision UI.
- `dotnet list reference`, madge, universal-ctags and ripgrep's `-C/--json` are standard tools, but their docs were not fetched in this pass. Verify the flags before scripting. No .NET equivalent of dependency-cruiser's `--affected` was confirmed.
- The code-review-graph token figures are self-reported against an unrealistic baseline. No independent benchmark was found.

## Q4. Which navigation affordances let the human jump from a decision card to the code, and what works in Claude Code today?

### Takeaway
Use three kinds of link, each with known limits:
1. **Editor URIs**: `vscode://file/<abs>:line:col`; `jetbrains://rider/navigate/reference?...` through Toolbox.
2. **SHA-pinned web permalinks**: GitHub `#Lx-Ly`; Azure DevOps `line/lineEnd/...` parameters.
3. **`claude-cli://` or `vscode://anthropic.claude-code/open` deep links** to come back and record the decision with a pre-filled prompt.

In the Claude Code terminal, OSC 8 hyperlinks appear only on tool-call headers, not in response text. In the VS Code integrated terminal, OSC 8 `file://` links do not open (upstream VS Code bug). The VS Code chat panel has had broken file-link regressions. Plain `path:line` text plus an HTML page with real links is the robust combination.

### Cited Findings
- [Docs] VS Code URL formats: `vscode://file/{full path to file}:line:column` (example: `vscode://file/c:/myProject/package.json:5:10`), `vscode-insiders://` for Insiders, and `vscode://vscode-remote/...` for remote. The CLI equivalent is `code --goto file:line[:char]`. These URLs work from browsers, file explorers and terminals. — [VS Code docs](https://code.visualstudio.com/docs/configure/command-line)
- [Community doc] JetBrains format: `jetbrains://{tool}/navigate/reference?project={name}&path={file}:{line}:{col}`, with tool tags such as `rider`, `idea` and `webstorm`. It needs the JetBrains **Toolbox** app, and navigation resolves inside an already-known project. — [Alan He, Dec 2024](https://medium.com/@alanhe421/understanding-jetbrains-url-scheme-4e315cd7cc63)
- [Docs] GitHub permalinks take the form `…/blob/<commit_SHA>/path#L10-L20`. They render as an embedded code snippet **only in the same repository** and only in comments, not in Markdown files. — [GitHub docs](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/creating-a-permanent-link-to-a-code-snippet)
- [Issue] Azure DevOps highlights a line only when the link carries `&line=7&lineEnd=8&lineStartColumn=1&lineEndColumn=1&lineStyle=plain` (the pattern ADO produces when copying a line link). `&line=` alone does nothing. — [dotnet/docfx #10447](https://github.com/dotnet/docfx/issues/10447)
- [Issue] Claude Code emits OSC 8 hyperlinks only in **tool-call headers** (Read/Write). File paths in plain or markdown response text are not clickable. Supporting terminals include Ghostty, WezTerm, Kitty and iTerm2. — [anthropics/claude-code #69008](https://github.com/anthropics/claude-code/issues/69008); related requests [#13008](https://github.com/anthropics/claude-code/issues/13008), [#27889](https://github.com/anthropics/claude-code/issues/27889)
- [Issue] Since v2.1.x, OSC 8 `file://` links render but do nothing when clicked in the VS Code integrated terminal ("No application found to open URL"), while a plain `file://` URL typed as text does open. The cause is an upstream VS Code bug (microsoft/vscode#242371). Closed "not planned". — [anthropics/claude-code #18717](https://github.com/anthropics/claude-code/issues/18717)
- [Issue] In the Claude Code VS Code extension chat panel, file references are styled as links but do not navigate, and sometimes show a bare filename. — [#72889](https://github.com/anthropics/claude-code/issues/72889). A separate regression report says the links stopped opening files from v2.1.178. — [#69240](https://github.com/anthropics/claude-code/issues/69240) (title only seen)
- [Docs] `claude-cli://open?q=<urlencoded prompt>&cwd=<abs path>|repo=<owner/name>` opens a new Claude Code terminal session with the prompt pre-filled but **not sent**, and shows a warning line: "Prompt from an external link". `q` is limited to 5,000 characters. The handler registers on macOS, Linux and Windows after the first interactive prompt. **GitHub strips non-http(s) schemes** in READMEs, issues, PRs and wikis, so put such links in a code block there. — [Claude Code deep links docs](https://code.claude.com/docs/en/deep-links)
- [Docs] The VS Code extension handles `vscode://anthropic.claude-code/open?prompt=…&session=<id>` and can resume a specific session. In Plan mode, VS Code opens the plan as a full Markdown document that accepts **inline comments** before Claude proceeds. — [Claude Code VS Code docs](https://code.claude.com/docs/en/vs-code)
- [Vendor] Bugbot's "Fix in Cursor" and "Fix in Web" links are an established pattern of deep-linking from a finding into an editor or agent. — [Cursor Bugbot docs](https://cursor.com/docs/bugbot)

### Inferences
- **Every evidence anchor on a card gets three forms**, emitted by the script:
  1. plain `relative/path.cs:142`, which survives any renderer, can be copied, and is readable by the model;
  2. an editor URI (`vscode://file/<abs>:142:1`, or `jetbrains://rider/...` when Rider or Toolbox is detected) for the HTML view;
  3. a SHA-pinned web permalink (GitHub `#L142-L150`, or ADO with the full `line/lineEnd/lineStartColumn/lineEndColumn/lineStyle` set) for anything shared or persisted in the ledger. Pinning to a SHA keeps the evidence valid after the code moves.
- **The "decide" action can be a deep link back into the agent.** `claude-cli://open?cwd=…&q=/decide%20D-012%20B` or `vscode://anthropic.claude-code/open?session=<id>&prompt=…` pre-fills the decision without auto-sending it. That fits a HITL gate, because the human still presses Enter. Mind the 5,000-character limit and GitHub's scheme stripping.
- **Terminal output.** Do not rely on OSC 8 in response text. Print `path:line` plainly. Terminals that auto-detect paths may make it clickable; this is unverified per terminal (see Gaps).
- **A local HTML page** (opened from disk in a browser) is the most reliable surface for clickable editor links. Browsers hand `vscode://` and `jetbrains://` to the OS, per the VS Code docs. Expect an "open external application" confirmation per scheme.
- In the VS Code extension, Plan-mode inline comments on a Markdown document are a native "review then approve" surface worth testing for decision cards.

### Gaps
- No documented URL scheme was found for opening a file at a line in full **Visual Studio** (as opposed to VS Code or Rider). This matters for .NET work.
- Unverified: whether a claude.ai-hosted artifact's sandbox allows navigation to `vscode://`, `jetbrains://` or `claude-cli://`; whether the Claude desktop app's markdown renderer makes custom-scheme links clickable; and whether VS Code's terminal link detector reliably opens `path:line:col` printed by Claude Code (VS Code issue #140780 was seen only as a title).
- The Azure DevOps base format (`?path=…&version=GC<sha>`) was not verified against Microsoft docs in this pass.
- The fix status of the VS Code chat-panel link regression (#69240, #72889) as of September 2026 is unknown.

## Q5. Which visual forms help (diagrams, dependency graphs, before/after code, option diffs, tables), and when?

### Takeaway
The measured evidence favours **code in the card** (multi-line code blocks, before/after excerpts, line-anchored) and **compact tables**. Diagrams have only *perceived* support, and teams switch off auto-generated sequence diagrams when they add noise. LLM-authored sequence diagrams have measurable logic and completeness errors. Use diagrams conditionally, prefer deterministically generated graphs, and never show a diagram for a trivial change.

### Cited Findings
- [Measured] AI review comments with a code-text ratio above 0.5, especially those with multi-line code blocks, were addressed noticeably more often. — [Sun, …, Treude 2026](https://ctreude.ca/pdf/does-ai-code-review-lead-2026.pdf)
- [Perceived; 394 practitioners] About half of sketches and diagrams were rated helpful for understanding the related code in future. Most are informal (UML mixed with other notation), and almost half stay in use for several weeks. — [Baltes & Diehl, FSE 2014](https://assets.empirical-software.engineering/pdf/se15-sketches.pdf)
- [Perceived] A structural graph of a merge request was rated clear by 78% and useful by 89%, but review performance was not measured. — [Fregnan et al., JSS 2023](https://www.sciencedirect.com/science/article/pii/S0164121222001820)
- [Perceived] Reviewers of LLM multi-file changes asked for architectural overviews and diagrams as the top level of a three-level workflow. — [Heander et al., 2026](https://arxiv.org/html/2606.01969v1)
- [Measured, model-side] MermaidSeqBench (NeurIPS 2025, 132 samples) found "significant capability gaps in accurately and reliably generating complex sequence diagrams", especially in logic, activation handling and error tracking. The models tested were open models (Llama, Qwen, Granite). — [MermaidSeqBench summary](https://lacuna.tiptreesystems.com/work/mermaidseqbench-an-evaluation-benchmark-for-llm-to-mermaid-sequence-diagram/wrk_9511b2042945c2a3903dc5802dc5f0be)
- [Vendor] Greptile picks the diagram type by change type (sequence for API flow, ER for schema, class for hierarchy, flow for business logic) and draws none for minimal changes. — [Greptile docs](https://www.greptile.com/docs/code-review/first-pr-review). CodeRabbit Change Stack: "not every layer gets one". — [CodeRabbit blog](https://www.coderabbit.ai/blog/introducing-atlas-the-first-ai-native-code-review-interface)
- [Practitioner] PostHog disabled CodeRabbit's sequence diagrams and changed-file summaries to reduce reading load. — [PostHog PR #100461](https://github.com/PostHog/posthog/pull/100461)
- [Docs] dependency-cruiser generates Mermaid dependency graphs deterministically and can highlight changed modules. Its Mermaid output is minified to fit within Mermaid's size limits. — [dependency-cruiser CLI](https://github.com/sverweij/dependency-cruiser/blob/main/doc/cli.md)
- [Measured, trust] Detailed explanations raised trust without changing time, and a bare verdict produced the lowest agreement. — [Gao et al. 2026](https://pith.science/paper/2607.24601)
- [Principle] Put compared items on the same screen at the same time. — [Baum et al. 2017](https://sback.it/publications/icsme2017.pdf)

### Inferences
**When to use each visual form**

| Form | Use when | Skip when | Est. tokens |
|---|---|---|---|
| Affected-files table (path, +/-, role, test?) | Always, if more than one file is involved | Single-line change | ~80–150 |
| Before/after excerpt (3–15 lines) | The decision concerns a concrete code shape | Pure policy or naming decisions | ~100–300 |
| Side-by-side "what each option changes" mini-diff | Options differ in code, not only in approach; keep to ≤2–3 options and one screen | Options differ only in process or cost | ~150–400 |
| Blast-radius list or graph (deterministic) | More than ~3 dependents or dependents across modules or projects; delocalized risk | Leaf or private change | List ~50–120; Mermaid ~120–250 |
| Sequence diagram of the affected path | Cross-component or async flow changes: API chain, message or event flow (for example command → aggregate → events → projections in an event-sourced design) | Intra-method change; any time the model would invent the flow without evidence | ~120–300 |
| Risk/reversibility badge plus one-line tally | Always | — | ~10–20 |

- Diagrams should be **generated from extracted facts** (the call hierarchy, dependency-cruiser, event handler registrations), or clearly labelled as illustrative, given MermaidSeqBench.
- Keep diagrams out of the conversation context. Render them in the HTML or artifact view and keep only the one-line takeaway in the chat.
- A numeric "review effort" score has no evidence behind it and was removed in practice. A reversibility tag ("two-way door / one-way door") is more decision-relevant. That tag is an inference, not an evidenced claim.

### Gaps
- No controlled experiment was found showing that sequence or dependency diagrams improve review or decision speed or accuracy. The evidence is perceived usefulness only.
- Mermaid rendering support was not verified in these surfaces: the Claude Code terminal, the VS Code extension chat panel, the Claude desktop app, and VS Code's built-in Markdown preview.
- No study was found on side-by-side option diffs specifically.

## Q6. For non-code decisions (Cowork), what is the equivalent, and does citation format speed verification?

### Takeaway
The equivalent of the code card is a short verbatim excerpt plus a deep link to the exact location in the source document, plus a small comparison table. Fine-grained (sub-sentence) attribution has been shown to cut human fact-verification time. Model-generated citations are often wrong (only 74.5% supported their sentence in 2023 generative search engines), so every excerpt must be checkable in one click.

### Cited Findings
- [Measured] In four generative search engines, only 51.5% of generated sentences were fully supported by their citations, and only 74.5% of citations supported the sentence they were attached to. — [Liu, Zhang & Liang, Findings of EMNLP 2023](https://aclanthology.org/2023.findings-emnlp.467/)
- [Measured] "Attribute First, then Generate" selects source spans first and uses them as fine-grained citations. It gives more concise citations and "significantly reduces the time required for fact verification by human assessors". The abstract gives no figure. — [Slobodkin et al., ACL 2024](https://aclanthology.org/2024.acl-long.182/)
- [Measured, model-side] Forcing sentence-level citations cost 2–97% (median 40%) in attribution quality compared with the best granularity. Paragraph-level was usually best across 8B–120B models. Human verification speed was *not* measured. — [Van Durme et al., Apr 2026](https://pith.science/paper/2604.01432)
- [Measured] When editing AI-written PR descriptions, developers most often *added* relevant links (22.7%) and templates (22.8%). — [Xiao et al., FSE 2024](https://tao-xiao.github.io/files/Copilot4PR_FSE_2024.pdf)

### Inferences
- **Cowork card anatomy:**
  - the question and trigger;
  - options in a table (criteria × options, ≤5 rows);
  - a recommendation;
  - per claim, a verbatim quote of about 1–2 sentences with a link to the exact location (page or section anchor, or a SharePoint or Word deep link) and the document's version or date.
- **Let the model work at paragraph granularity, but show the human the specific sentence.** This reconciles Van Durme (model quality) with Slobodkin (human verification speed).
- **Excerpts must be pulled verbatim by the tool**, not paraphrased by the model, because citation-support rates are poor (Liu 2023). A script can check that each quote exists in the source before the card is shown.

### Gaps
- No study was found comparing specific citation *formats* for human verification speed: inline quote, footnote, or hover preview.
- Deep-link formats for M365 content (a SharePoint or Word paragraph anchor, browser text fragments `#:~:text=`) were not researched in this pass.
- "Cited but Not Verified" (arXiv 2605.06635, on citations in deep-research agents) returned 403 and could not be read.
