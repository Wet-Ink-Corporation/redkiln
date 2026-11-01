# Decision-quality methods an AI agent can apply cheaply: decide-now tests, honest risk expression, evasion detection, decision-card fields, LLM confidence and default effects

Scope note: builds on the 2026-09-12 "Weigh-In" brief (00_prior_weigh-in_brief_2026-09-12.html). That brief already covers one-way/two-way doors, premortem, steelmanning/consider-the-opposite, overreliance HCI work (Buçinca 2021, Bansal 2021, Vasconcelos 2023, Fogliato 2022, Chiang 2024), sycophancy/position bias (Sharma 2024, Zheng 2024), Ambig-SWE over-asking, MADR 4 / Y-statements, Rust RFC resolve-by buckets, SATD TODO lifetimes, and the "hungry judge" ordering artefact. None of that is repeated below except as a cross-reference. Research date: 2026-09-26.

Evidence labels used below: **[evidence]** = peer-reviewed or large empirical study; **[preprint]** = unreviewed; **[practitioner]** = named practitioner opinion/heuristic, not tested; **[inference]** = my synthesis.

---

## 1. Decide now vs defer: value of information, cost of delay, last/most responsible moment, 70% rule, Duke's heuristics, real options

### Takeaway
Every credible framework reduces "should we wait?" to the same three checks: (a) name the specific information that would change the choice (if none exists, waiting has zero value), (b) can that information be got now at acceptable cost (if yes, get it, then decide), and (c) what waiting costs, including whether an option expires first. "Last responsible moment" is widely criticised as a license to procrastinate, and practitioners have replaced it with the "most responsible moment" and explicit option-expiry dates. These checks are cheap enough for an LLM to fill in for each question.

### Cited Findings
**Value of information (decision analysis) [evidence/theory]**
- Howard founded information value theory in 1966 ("Information Value Theory," *IEEE Trans. Systems Science and Cybernetics* 2(1):22–26). The value of information can never be negative, because the decider can always ignore it. The value of perfect information ("value of clairvoyance") is the upper bound on what any information-gathering is worth — [Wikipedia: Value of information](https://en.wikipedia.org/wiki/Value_of_information); [Semantic Scholar: Howard 1966](https://www.semanticscholar.org/paper/Information-Value-Theory-Howard/a7b3c2a88ca459d50010a33db8c2f113f1323e0c)
- Decision Quality (Stanford/SDG lineage) defines the stopping rule: "Decision quality is achieved when for each element the cost to obtain additional information or insight to improve its quality exceeds the added value." The framework also separates decision quality from outcome quality: a good decision can still turn out badly — [Wikipedia: Decision quality](https://en.wikipedia.org/wiki/Decision_quality)

**Cost of delay [practitioner, influential]**
- Reinertsen (*Principles of Product Development Flow*, 2009): "If you only quantify one thing, quantify the Cost of Delay." Cost of delay puts competing objectives into one economic unit — [Product Development 101 summary of Reinertsen](https://productdevelopment101.substack.com/p/productivity-in-product-development-4bb)
- The same secondary summary reports Reinertsen's claims that about 85% of product managers cannot quantify cost of delay and that their estimates differ by up to 50:1. It also reports his point that economic U-curves have a "long flat bottom," so rough estimates still improve decisions a great deal. These come from Reinertsen's own surveys and argument, reported second-hand; I did not check them against the book — [Product Development 101](https://productdevelopment101.substack.com/p/productivity-in-product-development-4bb)
- Fowler splits the cost of building a presumptive feature too early into cost of build, **cost of delay** (other work is pushed back), **cost of carry** (the extra complexity slows every later change) and **cost of repair** (the early build doesn't match what is learned later) — [Fowler, Yagni (2015-05-26)](https://martinfowler.com/bliki/Yagni.html)

**Last responsible moment and its critics [practitioner]**
- Poppendieck's definition: "delay commitment until the last responsible moment, that is, the moment at which failing to make a decision eliminates an important alternative" (quoted by Atwood, 2006-10-17) — [Coding Horror: The Last Responsible Moment](https://blog.codinghorror.com/the-last-responsible-moment/)
- Wirfs-Brock (2011-01-18) calls "decide at the LRM" an agile architecture myth. Decisions take time to spread across a team. Local choices ripple outward. Deferring can quietly set bad patterns in place. Deciding early stops teammates waiting on clarity. She proposes the "most responsible moment" instead — [Wirfs-Brock, Agile Architecture Myths #2](https://wirfs-brock.com/rebecca/blog/2011/01/18/agile-architecture-myths-2-architecture-decisions-should-be-made-at-the-last-responsible-moment/)
- Ben Morris (2014-02-21): "You can only tell when the last responsible moment has *passed*". In practice LRM turns into procrastination. Morris recommends designing in options (loose coupling) rather than deferring — [Ben Morris](https://www.ben-morris.com/lean-developments-last-responsible-moment-should-address-uncertainty-not-justify-procrastination/)
- Zimmermann's "Definition of Ready" for architectural decisions (START, 2023-12-01) includes **T = Time**: "The problem is both important and urgent; its Most Responsible Moment (MRM) has come" — [Zimmermann, A Definition of Ready for ADs](https://ozimmer.ch/practices/2023/12/01/ADDefinitionOfReady.html)
- Zimmermann's ITARC 2024 keynote says many significant decisions have an **early** MRM. These are the ones that set conceptual integrity, spawn many follow-on decisions, or carry high cost or many stakeholders. The risks at each extreme are: too early → "conceptual integrity at risk, technical debt"; too late → "tunnel or dead end, effort to revise" — [Zimmermann, Timing Architectural Decisions (ITARC 2024 slides)](https://ozimmer.ch/assets/presos/ZIO-ITARCKeynoteTADv101p.pdf)

**Real options [practitioner]**
- Matts & Maassen (InfoQ, 2007-06-08) give three rules: "Options have value." "Options expire." "Never commit early unless you know why." In practice: work out when the option expires by counting back from when implementation must start, and sometimes pay to keep more than one option open — [Matts & Maassen, "Real Options" Underlie Agile Practices](https://www.infoq.com/articles/real-options-enhance-agility/)

**Bezos 70% rule and disagree-and-commit [practitioner]**
- Bezos 2016 shareholder letter: "Most decisions should probably be made with somewhere around 70% of the information you wish you had. If you wait for 90%, in most cases, you're probably being slow." "Being wrong may be less costly than you think, whereas being slow is going to be expensive for sure." "If you're good at course correcting, being wrong may be less costly than you think." "Use the phrase 'disagree and commit.'" "Recognize true misalignment issues early and escalate them immediately" — [Amazon 2016 Letter to Shareholders](https://www.aboutamazon.com/news/company-news/2016-letter-to-shareholders)

**Annie Duke, *How to Decide* (2020) [practitioner, derived from decision science]**
- Stopping rule for gathering information: "Is there some information that I could find out that would change my mind?" If it is available at reasonable cost, get it; otherwise decide — [Duke, How to Decide excerpt (annieduke.com PDF)](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf)
- Happiness/impact test: "Ask yourself if the outcome of your decision, good or bad, will likely have a significant effect on your happiness in a year. Repeat for a month and a week." — [same PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf)
- Freeroll: "an asymmetry between the upside and downside because the potential losses are insignificant." A second source puts it as "Save time deciding whether to seize a freeroll; take time in deciding how to execute it" — [same PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf); [Cheng Nie notes](https://chengnie.com/notes/how-to-decide)
- Repeating options: "When the same type of decision comes up over and over again, you get repeated chances to choose options." — [same PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf)
- Only-option test: "If this were the only option I had, would I be happy with it?" If several options pass, the choice between them matters little. The "sheep in wolf's clothing" case follows from this: when options are close, the difficulty of choosing is itself the sign that you can choose fast — [same PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf); [Cheng Nie notes](https://chengnie.com/notes/how-to-decide)
- Quit-to-itiveness: "The lower the cost to quit, the faster you can go, because it's easier to unwind the decision." Menu strategy: "Spend your time on the initial sorting. Save your time on the picking." Decision stacking: "Finding ways to make low-impact, easy-to-quit decisions in advance of a high-impact, harder-to-quit decision." — [same PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf)
- Source conflict: a Fast Company adaptation (2020-11-06) credits these tests to *Thinking in Bets*. The annieduke.com excerpt places them in *How to Decide* (2020); treat that as authoritative — [Fast Company](https://www.fastcompany.com/90572468/3-simple-ways-to-become-less-anxious-and-more-decisive)

### Inferences
- **[inference] Operational decide-now test (per question, fillable by an LLM in about five fields):**
  1. **Flip condition.** "What specific fact, if learned, would change the recommendation?" Name it concretely: "whether tenant X needs per-record retention overrides," not "more requirements." If the agent cannot name one, the value of information is effectively zero (Howard; Duke). → **Decide now.**
  2. **Obtainable now?** Can that fact be got inside this session or cheaply: read the code, run a test, a spike under about an hour, a query, one question to a named person? If yes → **get it, then decide.** Do not surface the question. This matches the explore-first principle already in the prior brief.
  3. **Arrival vs expiry.** When will the fact arrive by itself (named event), and when does the option expire, meaning the point where work built on one branch makes the other expensive (Matts & Maassen; Poppendieck's "eliminates an important alternative")? If the fact arrives **after** expiry → **decide now**, because waiting buys nothing.
  4. **Cost of waiting in natural units.** What is blocked, what gets built on an assumption, and how much rework follows if the assumption is wrong (Fowler's carry and repair costs; Reinertsen)? If there is visible rework or blocked work and the flip condition is unlikely → **decide now.**
  5. **Closeness.** If the top options each pass the only-option test (Duke), the difference is small. → **Decide now**, record the pick, and mark it cheap to revisit.
  Deferral is legitimate only when **all** of these hold: a named flip condition exists; it cannot be got now; it will arrive before the option expires; interim work does not foreclose either branch; and the card records an owner, a trigger event, a resolve-by date and a **default that applies automatically** if the trigger does not fire. This last requirement matches the prior brief's owner/trigger/expiry principle.
- **[inference]** Zimmermann's early-MRM markers (sets conceptual integrity, many follow-on decisions, many stakeholders or high cost) make a good "decide-first, not later" flag. They reverse the naive reading of LRM for exactly the decisions Ryan cares about most: architecture foundations such as event model, aggregate boundaries, tenancy, and identifiers.
- **[inference]** Bezos's 70% figure is a heuristic, not a measured threshold. Encode it as a posture: "if the remaining uncertainty is mostly outside what can be learned before the option expires, decide." Do not use it as a numeric gate the model estimates.
- **[inference]** Duke's decision stacking fits agent work well. When a hard-to-reverse decision is blocked on information, the agent can propose a cheap, reversible probe (a spike, a thin vertical slice) that produces that information. This is a legitimate "not yet," but only when the probe is the next action rather than a promise.

### Gaps
- I found no empirical study that measures software teams' outcomes under LRM vs MRM vs early decision. All sources here are practitioner arguments.
- I did not verify Reinertsen's "85% cannot quantify / 50:1" claims against the book itself (only a secondary summary).
- Howard's original paper was not fetched directly; VoI properties are cited via encyclopaedic summaries of it.

---

## 2. Risk and impact expression without false precision

### Takeaway
Likelihood×impact matrices and ordinal 1–5 scores have documented mathematical and psychological failure modes. They cannot rank most pairs of risks, they can rank risks backwards, and people read verbal labels inconsistently. An LLM should not produce them. The non-bogus lightweight alternatives are: hard constraints as pass/fail screens (Kepner-Tregoe "musts"), consequences stated as concrete scenarios in natural units, reversibility × blast radius, "what would have to be true" conditions with the weakest one flagged, and ranges rather than point scores where numbers are needed.

### Cited Findings
- **Cox 2008 (Risk Analysis 28(2):497–512) [evidence, mathematical analysis]:**
  - **Poor resolution.** Typical risk matrices "can correctly and unambiguously compare only a small fraction (e.g., less than 10%) of randomly selected pairs of hazards" and "can assign identical ratings to quantitatively very different risks ('range compression')."
  - **Errors.** They "can mistakenly assign higher qualitative ratings to quantitatively smaller risks," and when frequency and severity are negatively correlated they can be "worse than useless," leading to worse-than-random decisions.
  - **Suboptimal allocation.** Resources "cannot be based on the categories provided by risk matrices."
  - **Ambiguous inputs and outputs.** "Different users may obtain opposite ratings of the same quantitative risks."
  — [PubMed abstract](https://pubmed.ncbi.nlm.nih.gov/18419665/); [Wiley](https://onlinelibrary.wiley.com/doi/abs/10.1111/j.1539-6924.2008.01030.x)
- **Hubbard & Evans 2010 (IBM J. Research & Development) [evidence review]:**
  - When experts state a 90% confidence interval, it contains the true value only 55–78% of the time. Calibration training moved participants from 47% to 80% of answers falling inside their stated 90% intervals.
  - Verbal probability labels were read as "a wide variety of possible values" even when definitions were supplied (IPCC research). This creates an "illusion of communication."
  - Ordinal scores are treated as ratio data ("very likely"=4 is taken as twice "not very likely"=2).
  - Range compression: in real data, 2–3 scale values covered most responses.
  - Scoring ignores correlations and cascades. They report that 5×5 matrices did not anticipate an actual NASA mission failure that Monte Carlo models flagged.
  - They recommend calibrated probability estimates plus Monte Carlo.
  — [Hubbard & Evans 2010 PDF](http://paci.com.au/downloads_public/risk/Douglas-Hubbard_May2010.pdf); [ACM DL](https://dl.acm.org/doi/10.5555/1850716.1850718)
- **Kepner-Tregoe decision analysis [practitioner method]:**
  - A decision statement comes first.
  - Objectives are split into **MUSTs**, which are mandatory and measurable. Any alternative that fails a MUST is eliminated (go/no-go).
  - **WANTs** are weighted 1–10, and alternatives are scored 1–10 against each WANT; weighted sums rank them.
  - A final **adverse-consequences** step lists potential problems for the top alternatives by probability and seriousness.
  — [Mindtools: Kepner-Tregoe Matrix](https://www.mindtools.com/atznth6/the-kepner-tregoe-matrix/)
- **Roger Martin, "What Would Have to Be True?" (2022-08-22) [practitioner]:** ask "What would have to be true?" instead of "What is true?". Laying out the logic "gives the two sides the chance of coming to agreement on the logic" before arguing over data. "If one or more of the features that have to be true either have ceased to hold true or our actions couldn't make true, it is a signal that you should revisit your strategy — right now" — [Roger Martin, Medium](https://rogermartin.medium.com/what-would-have-to-be-true-83dac5bd2189)
- Reinertsen's "long flat bottom" argument: rough economic estimates are enough to move a decision into the good region of a U-curve — [Product Development 101](https://productdevelopment101.substack.com/p/productivity-in-product-development-4bb)
- LLM-specific: LLM verbalized confidences are systematically inflated. Reasoning models often report >85% even when wrong (see Section 5). Any likelihood figure an LLM produces inherits that bias — [Mei et al. 2025](https://arxiv.org/html/2506.18183)

### Inferences
- **[inference] What an LLM can fill in without false precision:**
  1. **Hard constraints (MUSTs) as binary screens.** These come from prior decisions in force, stated non-negotiables, platform limits and compliance. They are checkable and can cite a source. An option that fails one is struck with the reason shown, not scored down.
  2. **Consequence if wrong, in natural units.** Examples: "requires a data migration of all event streams," "≈N files / M modules touched," "customer-visible," "breaks the public API," "rework of 1–3 days." Use ranges, not points. These can be checked against the repo.
  3. **Reversibility × blast radius.** This extends the prior brief's door type. Reversibility is the effort or mechanism to undo (config flip / code change / migration / contract with a third party). Blast radius is what is touched (one module / one service / all repos / clients). Both are categorical with concrete anchors, which avoids Cox's ambiguous-input problem.
  4. **"What would have to be true" for each option.** List 2–4 conditions and flag the one least likely to hold, together with its cheapest test (Martin). This replaces a "likelihood" score with a falsifiable condition.
  5. **1–3 named failure scenarios** ("this goes wrong if…") in place of a probability. If a likelihood word is used at all, pin it to a numeric range ("unlikely (<~20%)"), because Hubbard shows bare verbal labels are read inconsistently. Treat the LLM's number as uncalibrated.
- **[inference]** Do not have the skill output weighted-sum totals (Pugh/KT WANT scores) as the basis for the recommendation. The weights and 1–10 ratings are ordinal judgements (Hubbard's critique), and totals look more precise than they are. If weights matter, show the **swing**: "A wins unless you weight operability above time-to-market."
- **[inference]** Never colour-code a risk "heat" cell. Cox shows the categories can reverse the true order.

### Gaps
- I found no study specifically of LLM-generated risk ratings versus expert or quantitative baselines in software architecture.
- I did not find published evidence that "reversibility × blast radius" improves decision outcomes. It is a practitioner construct (Bezos door types plus SRE blast-radius practice), not a validated scale.
- The Pugh matrix was not researched separately. It shares the ordinal-scale issues Hubbard describes.

---

## 3. Evasion detection: recommendations that avoid the real decision

### Takeaway
There is no published checklist of "evasive recommendations," but each common evasion has its own evidence base:
- **"Make it configurable":** most configuration parameters are rarely set, and many config errors come from defaults left in place.
- **Feature flags as indecision:** flags are inventory with a carrying cost, and stale flags accumulate.
- **Abstraction "for flexibility":** speculative generality, and "the wrong abstraction" costs more than duplication.
- **TODO/"revisit later":** agent-authored self-admitted debt names problems without guidance on resolving them.
- **Uncorrected debt:** about 23% of AI-introduced issues are still present in the latest code version.
Fowler's debt quadrant gives the test that separates a legitimate deferral (deliberate and prudent, with a stated payoff) from an evasion.

### Cited Findings
**"Make it configurable" [evidence]**
- Xu et al., FSE 2015 ("Hey, you have given me too many knobs!"):
  - "Only a small percentage (6.1%–16.7%) of configuration parameters are set by the majority of users"; "up to 54.1% of parameters are rarely set by any user."
  - Up to 48.5% of configuration issues are users struggling to find or set parameters.
  - "up to 53.3% of configuration errors are introduced due to users' staying with default values incorrectly."
  - In one commercial system, 51.9% of parameters could be hidden or removed, affecting fewer than 1% of users.
  — [Never Work in Theory summary](https://neverworkintheory.org/2016/06/09/too-many-knobs.html); [ACM DL](https://dl.acm.org/doi/10.1145/2786805.2786852)

**Feature flags as deferred decisions [evidence + practitioner]**
- Uber's Piranha (ICSE-SEIP 2020):
  - It generated cleanup diffs for 1,381 stale flags, 17% of all flags.
  - 65% of diffs landed unchanged and more than 85% compiled and passed tests.
  - About 71k lines were deleted in total.
  - Flag debt "can adversely affect the overall reliability of the apps and increase their maintenance complexity," and the paper cites Knight Capital.
  — [Ramanathan et al., Piranha (PDF)](https://manu.sridharan.net/files/ICSE20-SEIP-Piranha.pdf)
- Hodgson (martinfowler.com, 2017-10-09): "Savvy teams view their Feature Toggles as inventory which comes with a carrying cost, and work to keep that inventory as low as possible." He recommends removal tasks, expiration dates, "time bombs" that fail tests past expiry, and caps on toggle count — [Hodgson, Feature Toggles](https://martinfowler.com/articles/feature-toggles.html)

**Abstraction "for flexibility" / speculative generality [practitioner]**
- Speculative Generality smell: "an unused class, method, field or parameter," created "'just in case' to support anticipated future features that never get implemented." Framework code is the main legitimate exception — [Refactoring.Guru](https://refactoring.guru/smells/speculative-generality)
- Metz (2016-01-20): "Duplication is far cheaper than the wrong abstraction." A premature abstraction collects parameters and conditionals with each new requirement. Her remedy is to inline it back into each caller: "When the abstraction is wrong, the fastest way forward is back" — [Sandi Metz, The Wrong Abstraction](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction)
- Fowler's important caveat: "Yagni only applies to capabilities built into the software to support a presumptive feature, it does not apply to effort to make the software easier to modify." Also: "If you do something for a future need that doesn't actually increase the complexity of the software, then there's no reason to invoke yagni" — [Fowler, Yagni](https://martinfowler.com/bliki/Yagni.html)

**Deliberate vs reckless debt [practitioner]**
- Fowler's Technical Debt Quadrant (2009-10-14) crosses deliberate/inadvertent with prudent/reckless. In the deliberate-prudent case "the team knows they are taking on a debt, and thus puts some thought as to whether the payoff for an earlier release is greater than the costs of paying it off." Fowler adds that low-interest debt in rarely touched code may never need repaying — [Fowler, TechnicalDebtQuadrant](https://martinfowler.com/bliki/TechnicalDebtQuadrant.html)

**Agent-authored SATD and deferred work (2025–2026) [evidence / preprint]**
- Brahmi, Ouni, Sayagh, Saied, "Characterizing Self-Admitted Technical Debt Generated by AI Coding Agents" (MSR 2026 Mining Challenge):
  - They analysed 525 agent-authored SATD comments from the AIDev dataset, covering Copilot, Cursor, Devin and Claude.
  - AI SATD is "slightly more technically detailed than human-authored SATD, yet both often describe problems without clear guidance on resolution."
  - 34 topics fall into 10 categories, and agents mostly document requirement- and design-related debt.
  - Categories specific to agents include infrastructure, pipelines, dependency management, and "requirement interpretation driven by developer prompts."
  — [MSR 2026 page](https://2026.msrconf.org/details/msr-2026-mining-challenge/28/Characterizing-Self-Admitted-Technical-Debt-Generated-by-AI-Coding-Agents); [ACM DOI](https://doi.org/10.1145/3793302.3793588)
- "TODO: Fix the Mess Gemini Created" (arXiv 2601.07786, Jan 2026) [preprint]:
  - Of 6,540 LLM-referencing code comments on GitHub (Nov 2022–Jul 2025), 81 also self-admit debt.
  - The themes are "postponed testing," "incomplete adaptation," and "limited understanding of AI-generated code."
  — [arXiv 2601.07786](https://arxiv.org/html/2601.07786v1)
- Liu, Widyasari, Zhao, Irsan, Chen, Lo, "Debt Behind the AI Boom" (arXiv 2603.28592v2, Apr 2026) [preprint]:
  - The corpus is 302,579 AI-attributed commits across 6,299 repos, from Copilot, Claude, Cursor, Gemini and Devin.
  - "More than 15% of commits from every AI coding assistant introduce at least one issue," ranging from 17.4% to 29.1% by tool.
  - 89.3% of issues are code smells, 6.0% correctness and 4.7% security.
  - "22.7% of tracked AI-introduced issues still survive at the latest version."
  — [arXiv 2603.28592](https://arxiv.org/html/2603.28592v2)
- Zhu, Tsantalis, Rigby (arXiv 2605.02741, May 2026) [preprint]:
  - Lines of code correlated with architectural smells at ρ=0.94 in multi-agent-generated applications.
  - The dominant patterns were long methods, god-class indicators (too many branches, high response-for-class) and scattered functionality.
  — [arXiv 2605.02741](https://arxiv.org/html/2605.02741v1)

**Phasing that moves the hard part out [practitioner]**
- Zimmermann's decision-ordering heuristics include "Worst first" and "Simple-complex-simple" — [Zimmermann ITARC 2024 slides](https://ozimmer.ch/assets/presos/ZIO-ITARCKeynoteTADv101p.pdf)

### Inferences
- **[inference] Evasion checklist.** Each item is a trigger phrase or pattern plus the question that clears it. A recommendation that trips a flag must either pass its clearing question in writing or be rewritten.
  1. **"Make it configurable / add an option / expose a setting."** Clear by naming the second concrete value and who needs it now. Otherwise pick the value (Xu 2015: most knobs are never touched, and left-at-default errors are common).
  2. **"Put it behind a feature flag."** Clear by naming the flag's kind (release/experiment/ops/permission), its removal owner and its removal trigger or date. A flag that selects between two designs is an undecided decision, not a rollout mechanism (Hodgson; Piranha).
  3. **"Add an interface / abstraction layer / strategy so we can swap later."** Clear by naming a second implementation needed within the planning horizon. Otherwise it is speculative generality. The Fowler caveat applies: refactoring that makes change easier without adding presumptive features is **not** an evasion.
  4. **"Revisit later / out of scope for now / phase 2 / follow-up ticket."** Clear with the decide-now test in Section 1: named flip condition, not obtainable now, arrives before the option expires, trigger + default + owner.
  5. **Phasing that puts the riskiest or least certain part last.** Clear by showing the hardest "what would have to be true" condition is tested in phase 1 ("worst first").
  6. **Introduces TODO/FIXME/HACK/workaround/"temporary."** Clear by classifying it with Fowler's quadrant. Only deliberate-prudent passes, and only with a written payoff statement and a repayment trigger. Agent SATD often names the problem without saying how to resolve it (Brahmi et al. 2026).
  7. **"Both / hybrid / support either."** Clear by showing that supporting both is itself the cheapest correct design. It is not correct if it is only there to avoid choosing.
  8. **"It depends on your needs / priorities."** Rewrite as a conditional recommendation ("Pick A unless X is true; X is [checkable]") using what-would-have-to-be-true.
  9. **"Needs more research / a spike."** Clear by naming the result that would flip the choice and the time-box, and by making the spike the next action.
  10. **Deferring verification ("tests later," "validate in staging").** Postponed testing is a documented GenAI-SATD theme (arXiv 2601.07786). Clear by saying what test would expose failure and why it cannot run now.
  11. **Recommendation relocates the question rather than resolving it.** The general test: after accepting, is the original question answered, or has it moved to a config file, a flag, an interface, a ticket, or a later phase?
- **[inference]** The quadrant is the right frame for judging honesty. A deliberate-prudent deferral names the debt, the payoff, the carrying cost and the repayment trigger. Everything else is either reckless or an evasion, even when phrased prudently.
- **[inference]** The survival figure (22.7% of AI-introduced issues still present, Liu et al. 2026) and the "problem without resolution guidance" finding (Brahmi et al. 2026) support a skill rule. Any agent-introduced deferral marker must become a decision-card item with a default and trigger, or be removed before hand-off.

### Gaps
- **No published checklist for evasive recommendations or "decision avoidance" in engineering advice was found.** The list above is my synthesis from single-pattern sources.
- I found no study specifically showing that LLM coding agents over-produce configurability, flags or speculative abstraction. The 2026 smell study emphasised long methods and god classes, not speculative generality. Whether agents over-abstract is currently anecdotal.
- I found no empirical work on how often LLM *recommendations* (as opposed to code) hedge or defer. This would be worth measuring in the skill's own eval.

---

## 4. Decision-quality frameworks and the minimal decision-card field set

### Takeaway
Decision Quality's six elements and Zimmermann's START/ecADR checklists fit together cleanly. Together with the common core of industry RFC/design-doc templates, they point to a small field set. Frame and constraints; at least two real alternatives with trade-offs; evidence; the value trade-off the human owns; reasoning with a flip condition; and a commitment step (enactment plus review trigger). DQ's "weakest link" rule gives the skill a quick check: score each element and fix the weakest before presenting.

### Cited Findings
- **Decision Quality (Spetzler, Winter & Meyer, *Decision Quality*, Wiley 2016; Howard/Raiffa lineage):** the six elements are "A useful frame," "Feasible and diverse alternatives," "Meaningful and reliable information," "Clear values, preferences, and trade-offs," "Logically sound reasoning," "Commitment to action." "The overall quality of the decision is limited by the weakest element." DQ is reached when each element's marginal improvement costs more than it adds — [Wikipedia: Decision quality](https://en.wikipedia.org/wiki/Decision_quality); [Carl Spetzler, CBS News](https://www.cbsnews.com/news/carl-spetzler-the-six-elements-of-a-good-decision/)
- **Zimmermann START (Definition of Ready, 2023):**
  - S: stakeholders known.
  - T: the most responsible moment has come.
  - A: "At least two design alternatives… have been identified. Their pros and cons are already known, or it is possible and planned to find out."
  - R: "Problem context as well as requirements and other decision drivers (aka criteria) have been analyzed and documented."
  - T: template chosen.
  — [Zimmermann 2023](https://ozimmer.ch/practices/2023/12/01/ADDefinitionOfReady.html)
- **Zimmermann ecADR (Definition of Done):** Evidence that the chosen option will work; Criteria for selection; Agreement among stakeholders; Documentation of outcome and rationale; Realization/Review plan — [Zimmermann 2023](https://ozimmer.ch/practices/2023/12/01/ADDefinitionOfReady.html)
- **Google design docs (Ubl, 2020-07-06):**
  - "Alternatives considered" lists "alternative designs that would have reasonably achieved similar outcomes," focusing "on the trade-offs that each respective design makes."
  - The doc is "*the place to write down the trade-offs*."
  - Its value is "early identification of design issues when making changes is still cheap."
  - If a doc only describes implementation "without going into trade-offs, alternatives, and explaining decision making," skip it.
  — [Malte Ubl, Design Docs at Google](https://www.industrialempathy.com/posts/design-docs-at-google/)
- **Industry RFC/design-doc templates (Orosz, 2022, updated 2024):** more than 100 companies are listed (Google, Uber, Amazon, Stripe, Shopify, Spotify, GitLab, etc.). Recurring sections are Context/Background, Proposal/Decision, Alternatives Considered, Goals/Non-Goals, Risks and Open Questions, and some firms (HashiCorp, Stedi) require explicit approval — [Pragmatic Engineer: Companies Using RFCs or Design Docs](https://blog.pragmaticengineer.com/rfcs-and-design-docs/)
- **GitLab TeamOps "Decision Velocity":** "As many decisions as possible should be made by the person doing the work (the DRI), not by their manager"; "A bias for action accelerates ideation, collaboration, and execution better than alignment and consensus" — [GitLab Handbook: Decision Velocity](https://handbook.gitlab.com/teamops/decision-velocity/)
- **Kepner-Tregoe:** the decision statement "discuss[es] the action that is required and the result that is desired"; MUST/WANT split; adverse-consequences step — [Mindtools](https://www.mindtools.com/atznth6/the-kepner-tregoe-matrix/)

### Inferences
- **[inference] Minimal decision-card fields** (DQ element / ecADR letter in brackets). Items marked * are new relative to the prior brief's D-0001 card.
  1. **Question**: a single interrogative, plus the desired result (KT decision statement). [Frame]
  2. **Constraints in force (MUSTs)**: prior decisions, non-negotiables, with links. Options failing a MUST are shown struck through. [Frame / Values]
  3. ***Timing verdict**: "Decide now" or "Decide after ⟨named fact⟩ by ⟨trigger/date⟩; default if not decided: ⟨option⟩; owner." Include the flip condition and the option-expiry point. [Frame / Commitment]
  4. **Door type**: reversibility × blast radius with concrete anchors. [Information]
  5. **Options (2–4, including do-nothing/status quo)**, each with: *what would have to be true*, consequence-if-wrong in natural units, evidence links, steelman. [Alternatives / Information]
  6. **Recommendation + rationale + *"what would change this"*** (the flip condition), with calibrated wording (Section 5). [Reasoning / ecADR-C]
  7. ***Value trade-off the human owns**: one line naming the preference the human is being asked to confirm ("You are trading X for Y"). [Values]
  8. ***Evasion check**: pass, or list the flags tripped with their clearing answers (Section 3). [Reasoning]
  9. **Commitment**: the enactment step on accept (file/PR/ADR write-back) and the review trigger. [Commitment / ecADR-R]
- **[inference]** Use DQ's weakest-link rule as a self-check before presenting. The agent rates each of the six elements as adequate or weak and fixes the weakest (usually alternatives or information) instead of padding the others. This is cheap, and it matches DQ's own stopping rule.
- **[inference]** Field 7 matters because of the Vaccaro meta-analysis (Section 5). Humans add most value on the values/trade-off element. The card should make that the thing the human confirms, not re-derive the technical facts.

### Gaps
- I did not find published decision-record templates specific to Stripe, Shopify, Spotify or Uber beyond the Orosz summary. Company-specific minimal fields could not be verified from primary sources.
- I found no empirical test of the DQ six-element checklist as a quick team checklist, as opposed to a consulting framework.

---

## 5. LLM-specific (2024–2026): confidence reliability, whether AI option analyses help, and default/nudge effects of pre-selecting the recommendation

### Takeaway
- **Verbalized LLM confidence** is inflated, and more reasoning makes it worse. Consistency across samples is the more reliable signal.
- **Presentation drives trust.** Longer explanations raise human trust without raising accuracy. Honest first-person uncertainty reduces overreliance.
- **Combining humans and AI** gives worse results on average in decision tasks, especially when the AI is the stronger party.
- **Evidence-for-and-against designs** have mixed support: one study showed better accuracy at a large time cost, another showed no improvement.
- **Pre-selecting the recommended option** is a default. Defaults are among the few robust nudges and work partly through perceived endorsement, and disclosing a default does not weaken it.
So a pre-selected recommendation stays honest only through its own quality and visible flip conditions, not through disclosure.

### Cited Findings
**Verbalized confidence and calibration**
- Xiong et al., ICLR 2024 [evidence]: LLMs tend to be overconfident when verbalizing confidence, "potentially mimicking how humans verbalize confidence." Human-inspired prompts, sampling multiple responses to check consistency, and better aggregation all improve calibration and failure prediction. Larger models do better, no single method wins everywhere, and all struggle on specialist tasks — [arXiv 2306.13063](https://arxiv.org/abs/2306.13063)
- Mei et al. (Princeton, 2025-07) [preprint]:
  - Reasoning models are "typically overconfident, with self-verbalized confidence estimates often greater than 85% particularly for incorrect responses."
  - They "become even more overconfident with deeper reasoning."
  - "Introspective" UQ improved o3-mini and DeepSeek-R1, but "Claude 3.7 Sonnet becomes more poorly calibrated."
  — [arXiv 2506.18183](https://arxiv.org/html/2506.18183)
- Farquhar et al., *Nature* 2024 [evidence]: semantic entropy. The method samples several answers, clusters them by meaning and computes entropy across the clusters. It detects "confabulations" without task-specific data — [Nature](https://www.nature.com/articles/s41586-024-07421-0)
- Hsiao, 2026-09-10 [preprint, single author, very recent]:
  - For LLM-as-judge on "post-2025" top proprietary models, verbalized confidence is now a *better* soft signal than log-probabilities, which the author calls a "compatibility shift."
  - Adding an "overconfidence advisory" and "self-debate" improved calibration with little accuracy cost on newer models.
  - This is unreplicated and only two weeks old.
  — [arXiv 2609.10996](https://arxiv.org/abs/2609.10996)
- Steyvers et al., *Nature Machine Intelligence* 2025 [evidence]:
  - There is a "calibration gap" between human confidence in LLM answers and the model's own confidence.
  - "longer explanations increased user confidence, even when the extra length did not improve answer accuracy."
  - Changing explanation wording to reflect the model's internal confidence narrowed both the calibration and discrimination gaps.
  — [arXiv 2401.13835](https://arxiv.org/abs/2401.13835); [Nature MI](https://www.nature.com/articles/s42256-024-00976-7)
- Kim et al., FAccT 2024 [evidence, N=404]:
  - First-person uncertainty ("I'm not sure, but…") cut agreement with the AI from 80.9% to 74.8% and raised task accuracy from 63.9% to 72.8%.
  - General-perspective phrasing ("It's not clear…") was not significant.
  - First-person uncertainty also lowered users' intention to use the system.
  - The No-AI baseline reached 74.2% accuracy.
  — [ar5iv 2405.00623](https://ar5iv.labs.arxiv.org/html/2405.00623); [ACM DL](https://dl.acm.org/doi/10.1145/3630106.3658941)

**Do AI option analyses improve human decisions?**
- Vaccaro, Almaatouq & Malone, *Nature Human Behaviour* 2024 [meta-analysis, 106 studies, 370 effect sizes]:
  - Human–AI combinations did "significantly worse than the best of humans or AI alone (Hedges' g = −0.23)."
  - Performance was lost "in tasks that involved making decisions" and gained in content creation.
  - "When humans outperformed AI alone, we found performance gains in the combination, but when AI outperformed humans alone, we found losses."
  — [Nature Human Behaviour](https://www.nature.com/articles/s41562-024-02024-1)
- Miller, FAccT 2023 [position paper]: "Evaluative AI" — tools "provide evidence for and against decisions made by people, rather than provide recommendations to accept or reject" — [arXiv 2302.12389](https://arxiv.org/abs/2302.12389)
- "An Empirical Examination of the Evaluative AI Framework" (arXiv 2411.08583; published in *IJHCI* 2025) [evidence]: "no significant improvement in decision-making performance and limited user engagement with the evidence provided" — [arXiv 2411.08583](https://arxiv.org/abs/2411.08583); [T&F](https://www.tandfonline.com/doi/full/10.1080/10447318.2025.2581260)
- Ma et al., CHI 2025, "Deliberative AI" [evidence]:
  - Dimension-level human–AI deliberation over disagreements, compared with a conventional explainable-AI assistant.
  - Accuracy rose from 52.4% to 59.8% and over-reliance fell from 65% to 47%, with no rise in under-reliance.
  - Median time per decision went from 33 s to **236 s**.
  — [arXiv 2403.16812](https://arxiv.org/html/2403.16812v2); [ACM DL](https://dl.acm.org/doi/10.1145/3706598.3713423)
- Hunter, Moulange, Bernardi, Stein, 2024 (NeurIPS 2024 workshop): "reliance drills." Organisations deliberately inject known-wrong AI advice to check whether humans catch it, as a routine risk-management practice — [arXiv 2409.14055](https://arxiv.org/abs/2409.14055)

**Defaults, nudges and pre-selection**
- Jachimowicz et al., *Behavioural Public Policy* 2019 [meta-analysis]:
  - 58 studies, pooled n=73,675, d=0.68 (95% CI 0.53–0.83), with very high heterogeneity (I²≈98%).
  - Defaults are "more effective when they operate through endorsement" (seen as the choice architect's recommendation) "or endowment" (seen as the status quo).
  - Ease/effort was **not** a significant moderator.
  — [Cambridge Core](https://www.cambridge.org/core/journals/behavioural-public-policy/article/when-and-why-defaults-influence-decisions-a-metaanalysis-of-default-effects/67AF6972CFB52698A60B6BD94B70C2C0)
- Johnson & Goldstein, *Science* 2003: opt-out organ-donor countries show much higher effective consent than opt-in countries. This is the canonical default-effect study (exact rates not re-verified here) — [Science](https://www.science.org/doi/10.1126/science.1091721)
- Bruns et al., *J. Economic Psychology* 2018 [evidence]: telling people about a default's potential influence, its purpose, or both did "not significantly influence the default effect" — [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0167487017307845)
- Maier et al., *PNAS* 2022 [evidence]: after correcting Mertens et al.'s nudge meta-analysis for publication bias, "no evidence remains that nudges are effective as tools for behaviour change." Heterogeneity remains, and evidence for "structure" interventions (the category containing defaults) is "undecided" — [PNAS](https://www.pnas.org/doi/10.1073/pnas.2200300119)

### Inferences
- **[inference] Confidence rules for the skill:**
  - (a) Never show a single-shot verbalized percentage.
  - (b) Get confidence from agreement: re-run the recommendation 3–5 times with option order permuted and framing neutralised (prior brief), or using an independent judge. Report "stable across k/n runs" or "unstable."
  - (c) Put that into first-person wording only when it is real ("I'm not confident here because runs split between A and C"). Kim 2024 shows this reduces overreliance.
  - (d) Keep rationale length proportional to confidence and short by default. Steyvers 2025 shows length alone inflates trust.
  - (e) Do not treat longer model "thinking" as more confidence (Mei 2025).
  - The Hsiao 2026 result is a reason to re-test verbalized confidence on current Claude models in the skill's eval, not a reason to trust it now.
- **[inference] Where the human adds value.** Vaccaro 2024 suggests combining human and AI hurts when the AI is stronger. For facts recoverable from code, tests and docs, the agent should settle them (and two-way doors should be auto-decided, per the prior brief). The human's attention should go to the values/trade-off element and to one-way doors, where human context is the stronger input.
- **[inference] Pre-selection and honesty.** A pre-selected "(Recommended)" option acts as a default through endorsement. That is the strongest default mechanism (Jachimowicz), and disclosing it does not weaken it (Bruns). Honesty therefore cannot come from a disclaimer. Ryan wants to accept fast most of the time, so the honest design keeps pre-selection but earns it:
  - (1) Pre-select only when the recommendation passed the evasion check and was stable across permuted runs.
  - (2) Always show the flip condition next to the pick, so accepting means accepting "A unless X."
  - (3) For one-way doors or unstable recommendations, show the options first, per the prior brief's reveal-on-demand principle.
  - (4) Track override and post-hoc reversal rates.
  - (5) Optionally run opt-in reliance drills (Hunter 2024). These are rare, labelled after the fact, and seed a deliberately weaker recommendation to check that review is still happening. They only work with Ryan's advance consent because he is both designer and user.
- **[inference] Deliberation is costly.** Deliberative AI's accuracy gain cost about 7× the time (Ma 2025) and evaluative AI showed no gain (2025). Heavyweight deliberation should be reserved for one-way doors, which conflicts with a default of "accept fast."

### Gaps
- I found no study of *pre-selected AI recommendations* specifically, meaning default effects of a pre-checked AI option in a UI, as opposed to classic policy defaults. The inference relies on general default research.
- I found no 2025–2026 study that directly tests AI-generated *multi-option decision cards* (options + risks + recommendation) for software/architecture decisions. The evidence comes from other decision tasks (classification, forecasting, medical).
- The Xiong 2024 AUROC comparison between white-box and black-box methods was ambiguous in the fetched summary and is omitted.
- The "contrastive explanations" line of work (2025) was not reached within budget.

---

## 6. Sequencing multiple decisions: dependency ordering, batching, decision-fatigue evidence

### Takeaway
Order decisions so that those with many follow-on decisions and those that set conceptual integrity come first ("early MRM," "worst first"). Use cheap reversible decisions to buy information for expensive ones (Duke's decision stacking). Batch items to cut context switches, not to avoid "depletion": the ego-depletion basis for decision fatigue failed a 36-lab preregistered test.

### Cited Findings
- Zimmermann (ITARC 2024): decisions with many follow-on decisions and wide ripple effects have an early MRM. Ordering heuristics are "Worst first," "Simple-complex-simple," value-vs-effort (start high-value/low-effort, then high/medium, then high/high), and running an architectural-significance test first — [Zimmermann ITARC 2024 slides](https://ozimmer.ch/assets/presos/ZIO-ITARCKeynoteTADv101p.pdf)
- Duke: decision stacking — "Finding ways to make low-impact, easy-to-quit decisions in advance of a high-impact, harder-to-quit decision." Menu strategy — "Spend your time on the initial sorting. Save your time on the picking." — [Duke excerpt PDF](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf)
- Bezos 2016: "Recognize true misalignment issues early and escalate them immediately" — [Amazon 2016 letter](https://www.aboutamazon.com/news/company-news/2016-letter-to-shareholders)
- Vohs et al., *Psychological Science* 2021 [evidence]: a multisite preregistered ego-depletion test across 36 labs (N=3,531) found d=0.06 (not significant). Bayesian analysis found the data "4 times more likely under the null than the alternative" — [SAGE](https://journals.sagepub.com/doi/abs/10.1177/0956797621989733)
- The prior brief already covers the "hungry judge" ordering artefact and the Iqbal & Bailey breakpoint-delivery evidence. Not repeated here.

### Inferences
- **[inference] Sequencing rule for a batch of open decisions:**
  - (1) Build the dependency list. For each card, record which other cards' options it constrains or makes moot, using a simple "blocks/blocked-by" link.
  - (2) Present upstream decisions first: many dependents, one-way door, conceptual integrity. Downstream cards whose options change depending on an upstream choice should be *regenerated* after the upstream decision, not presented in parallel.
  - (3) Within a tier, put "worst first," meaning the one with the weakest what-would-have-to-be-true condition.
  - (4) Auto-decide two-way doors and freerolls without presenting them.
  - (5) Group the rest by shared context so the human loads each context once.
- **[inference]** The justification for batching and caps is attention and context-switch cost plus habituation (prior brief), not ego depletion. The skill should not state "decision fatigue" as an established mechanism.

### Gaps
- Kruchten's 2004 ontology of decision relationships (constrains/forbids/enables/subsumes/conflicts-with) was not verified from a primary source within budget. It is a likely formal basis for the "blocks/blocked-by" link types.
- I found no empirical study on optimal ordering of multiple architecture decisions for a single decider.
- Observational "decision fatigue" findings in clinical settings (e.g., prescribing later in the day) were not researched. The replication picture here rests on the ego-depletion literature plus the prior brief's judge-study critique.
