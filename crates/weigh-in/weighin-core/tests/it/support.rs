//! A deterministic PRNG and generators of adversarial text, so the properties need no dependency
//! beyond std. Every property runs from a fixed seed, so a failure reproduces exactly; the case
//! index is in the assertion message.

/// Cases per property: enough to reach the odd corners, few enough to keep the suite fast.
pub(crate) const CASES: u64 = 3000;

/// What a property or fixture with fallible setup returns, so the setup propagates with `?`
/// (TST-04). A property with no fallible step returns `()` (`clippy::unnecessary_wraps`).
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// splitmix64 (Steele, Lea and Flood): tiny, fast and good enough to drive generators.
#[derive(Debug, Clone)]
pub(crate) struct Rng(u64);

impl Rng {
    /// A generator for case `case` of the property seeded with `seed`.
    pub(crate) fn new(seed: u64, case: u64) -> Self {
        let mut rng = Self(seed ^ case.wrapping_mul(0xA076_1D64_78BD_642F));
        rng.next_u64();
        rng
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`, or 0 when `n` is 0.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        self.next_u64()
            .checked_rem(n64)
            .and_then(|r| usize::try_from(r).ok())
            .unwrap_or(0)
    }

    /// True with probability `1 / n`.
    pub(crate) fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    /// One element of a non-empty slice.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let i = self.below(items.len());
        items
            .get(i)
            .expect("pick is only called on non-empty slices")
    }

    /// Any Unicode scalar value, biased towards the low planes where the odd characters live.
    pub(crate) fn any_char(&mut self) -> char {
        let limit: u32 = match self.below(4) {
            0 => 0x80,
            1 => 0x800,
            2 => 0x1_0000,
            _ => 0x11_0000,
        };
        let raw = u32::try_from(self.next_u64() % u64::from(limit)).unwrap_or(0);
        char::from_u32(raw).unwrap_or('\u{fffd}')
    }
}

/// Fragments that have broken line-oriented parsers and front matter before: fences, comment
/// marks, separators, line breaks of every kind, NUL, combining marks, case-mapping oddities.
pub(crate) const FRAGMENTS: &[&str] = &[
    "",
    " ",
    "  ",
    "\t",
    "\n",
    "\r\n",
    "\r",
    "\u{0}",
    "\u{7f}",
    "\u{85}",
    "\u{2028}",
    "\u{2029}",
    "\u{feff}",
    "\u{200b}",
    "\u{fffe}",
    "\u{ffff}",
    "`",
    "```",
    "````",
    "~~~",
    "```weigh-in",
    "~~~weighin",
    "```rust",
    "#",
    " #",
    "# ",
    ":",
    ": ",
    "q:",
    "q: ",
    "kind:",
    "door:",
    "authority:",
    "did:",
    "none",
    "-",
    "---",
    "\"",
    "'",
    "\\",
    "\\u0000",
    "{",
    "}",
    "[",
    "]",
    "e\u{301}",
    "\u{301}",
    "İ",
    "Σ",
    "ß",
    "日本語",
    "🦀",
    "C#",
    "C++",
    "wi-",
    "wi-3fa9c1",
    "WI-ABC",
    "?",
    ".",
    "!",
    "a",
    "Z",
    "0",
    "word",
];

/// An adversarial string: fragments and arbitrary characters, now and then a very long line.
pub(crate) fn adversarial(rng: &mut Rng, max_parts: usize) -> String {
    let mut s = String::new();
    let parts = rng.below(max_parts.saturating_add(1));
    for _ in 0..parts {
        match rng.below(10) {
            0..=5 => s.push_str(rng.pick(FRAGMENTS)),
            6..=8 => s.push(rng.any_char()),
            _ => {
                if rng.one_in(8) {
                    let c = rng.any_char();
                    s.extend(std::iter::repeat_n(c, 2000 + rng.below(3000)));
                } else {
                    s.push_str(rng.pick(FRAGMENTS));
                }
            }
        }
    }
    s
}

/// An adversarial string with no `\n` or `\r`, so it stays on one line.
pub(crate) fn one_line(rng: &mut Rng, max_parts: usize) -> String {
    adversarial(rng, max_parts).replace(['\n', '\r'], " ")
}

/// Text built line by line from lines that look like marker syntax, with fences in between.
pub(crate) fn marker_soup(rng: &mut Rng) -> String {
    const LINES: &[&str] = &[
        "```weigh-in",
        "````weigh-in",
        "~~~ weighin",
        "  ```weigh_in extra",
        "```",
        "````",
        "~~~",
        "```rust",
        "q: a question?",
        "question: another?",
        "kind: call",
        "door: one-way",
        "authority: judgment",
        "did: blocked",
        "blocks: none",
        "  continued",
        "# a comment",
        "",
        "owner: someone # note",
    ];
    let mut lines = Vec::new();
    for _ in 0..rng.below(24) {
        if rng.one_in(3) {
            lines.push(one_line(rng, 6));
        } else {
            lines.push((*rng.pick(LINES)).to_owned());
        }
    }
    let sep = if rng.one_in(2) { "\n" } else { "\r\n" };
    lines.join(sep)
}
