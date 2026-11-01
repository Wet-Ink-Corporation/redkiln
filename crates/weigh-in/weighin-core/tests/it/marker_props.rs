//! Properties of the `weigh-in` marker parser.

use crate::support::{CASES, Rng, adversarial, marker_soup, one_line};
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use weighin_core::marker::{Authority, Door, Kind, Marker, authority_tripwire, parse_markers};

/// Keys the parser gives a meaning to; generated unknown keys must avoid them.
const KNOWN_KEYS: &[&str] = &[
    "q",
    "question",
    "kind",
    "door",
    "authority",
    "did",
    "flip",
    "at",
    "sym",
    "symbol",
    "blocks",
];

/// A value the parser keeps verbatim once trimmed: one line, non-empty, not `none` or `-`, and
/// without ` #`, which starts a trailing comment.
fn value(rng: &mut Rng) -> String {
    loop {
        let mut v = one_line(rng, 8);
        while v.contains(" #") {
            v = v.replace(" #", "#");
        }
        let v = v.trim();
        if !v.is_empty() && !v.eq_ignore_ascii_case("none") && v != "-" {
            return v.to_owned();
        }
    }
}

/// An unknown key: ASCII alphanumerics, `_` and `-`, in any case.
fn unknown_key(rng: &mut Rng) -> String {
    const KEY_CHARS: &[char] = &['a', 'b', 'x', 'Z', '0', '9', '_', '-'];
    loop {
        let key: String = (0..=rng.below(6)).map(|_| *rng.pick(KEY_CHARS)).collect();
        if !KNOWN_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
            return key;
        }
    }
}

/// The written spelling of an enum value: as rendered, or shouted.
fn spelled(rng: &mut Rng, name: &str) -> String {
    if rng.one_in(3) {
        name.to_ascii_uppercase()
    } else {
        name.to_owned()
    }
}

/// An optional field: the value the parser should report, and the line that writes it.
fn optional(rng: &mut Rng, key: &str, lines: &mut Vec<String>) -> Option<String> {
    match rng.below(4) {
        0 => None,
        1 => {
            lines.push(format!("{key}: {}", spelled(rng, "none")));
            None
        }
        _ => {
            let v = value(rng);
            lines.push(format!(
                "{key}:{}{v}",
                if rng.one_in(2) { " " } else { "  " }
            ));
            Some(v)
        }
    }
}

/// A well-formed block body, its lines shuffled and padded with blank and comment lines, and the
/// marker it must parse to, `raw` included.
fn well_formed(rng: &mut Rng) -> (Vec<String>, Marker) {
    let mut lines = Vec::new();
    let question = value(rng);
    let q_key = if rng.one_in(2) { "q" } else { "question" };
    lines.push(format!("{q_key}: {question}"));
    let kind = *rng.pick(&[Kind::Question, Kind::Assumption, Kind::Deferral, Kind::Call]);
    if !(kind == Kind::Question && rng.one_in(2)) {
        lines.push(format!("kind: {}", spelled(rng, kind.as_str())));
    }
    let door = *rng.pick(&[Door::OneWay, Door::TwoWay, Door::Unsure]);
    if !(door == Door::Unsure && rng.one_in(2)) {
        lines.push(format!("door: {}", spelled(rng, door.as_str())));
    }
    let authority = *rng.pick(&[Authority::Authority, Authority::Judgment]);
    if !(authority == Authority::Authority && rng.one_in(2)) {
        lines.push(format!("authority: {}", spelled(rng, authority.as_str())));
    }
    let did = optional(rng, "did", &mut lines);
    let flip = optional(rng, "flip", &mut lines);
    let at = optional(rng, "at", &mut lines);
    let sym_key = if rng.one_in(2) { "sym" } else { "symbol" };
    let sym = optional(rng, sym_key, &mut lines);
    let blocks = optional(rng, "blocks", &mut lines);
    let mut extra = BTreeMap::new();
    for _ in 0..rng.below(3) {
        let key = unknown_key(rng);
        let v = value(rng);
        let line = format!("{key}: {v}");
        if let Entry::Vacant(slot) = extra.entry(key.to_ascii_lowercase()) {
            slot.insert(v);
            lines.push(line);
        }
    }
    shuffle(rng, &mut lines);
    for _ in 0..rng.below(3) {
        let at = rng.below(lines.len() + 1);
        let filler = if rng.one_in(2) {
            String::new()
        } else {
            format!("# {}", one_line(rng, 4))
        };
        lines.insert(at, filler);
    }
    let marker = Marker {
        kind,
        door,
        authority,
        question,
        did,
        flip,
        at,
        sym,
        blocks,
        extra,
        raw: lines.join("\n"),
    };
    (lines, marker)
}

fn shuffle<T>(rng: &mut Rng, items: &mut [T]) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
}

/// An opening and a matching closing fence for a `weigh-in` block.
fn fences(rng: &mut Rng) -> (String, String) {
    let mark = *rng.pick(&["```", "````", "~~~", "~~~~"]);
    let info = *rng.pick(&["weigh-in", "weighin", "weigh_in", "Weigh-In", "WEIGHIN"]);
    let indent = *rng.pick(&["", " ", "  "]);
    let gap = *rng.pick(&["", " "]);
    let tail = *rng.pick(&["", " extra words"]);
    (
        format!("{indent}{mark}{gap}{info}{tail}"),
        format!("{indent}{mark}"),
    )
}

/// A line of prose that can never open or close a fence.
fn prose(rng: &mut Rng) -> String {
    format!("> {}", one_line(rng, 10))
}

fn without_raw(mut marker: Marker) -> Marker {
    marker.raw.clear();
    marker
}

#[test]
fn parsing_never_panics_on_adversarial_text() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x3a_0001, case);
        let text = if rng.one_in(2) {
            adversarial(&mut rng, 60)
        } else {
            marker_soup(&mut rng)
        };

        for marker in parse_markers(&text) {
            assert!(!marker.question.is_empty(), "case {case}: empty question");
            assert!(
                marker.question.trim() == marker.question,
                "case {case}: question is trimmed"
            );
            assert!(
                !marker.is_blocked() || marker.did.is_some(),
                "case {case}: blocked needs a did"
            );
        }
        if let Some(word) = authority_tripwire(&text) {
            assert!(
                text.to_lowercase().contains(word),
                "case {case}: tripwire {word} is in the text"
            );
        }
        let shouted = text.to_ascii_uppercase();
        assert_eq!(
            Kind::parse(&shouted),
            Kind::parse(&text),
            "case {case}: kind"
        );
        assert_eq!(
            Door::parse(&shouted),
            Door::parse(&text),
            "case {case}: door"
        );
        assert_eq!(
            Authority::parse(&shouted),
            Authority::parse(&text),
            "case {case}: authority"
        );
    }
}

#[test]
fn a_rendered_block_parses_back_to_its_fields() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x3a_0002, case);
        let (body, expected) = well_formed(&mut rng);
        let (open, close) = fences(&mut rng);
        let text = format!(
            "{}\n{open}\n{}\n{close}\n{}",
            prose(&mut rng),
            body.join("\n"),
            prose(&mut rng)
        );

        let parsed = parse_markers(&text);

        assert_eq!(parsed, vec![expected], "case {case}: {text:?}");
    }
}

#[test]
fn blocks_never_share_fields() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x3a_0003, case);
        let mut text = Vec::new();
        let mut expected = Vec::new();
        for _ in 0..=rng.below(4) {
            text.push(prose(&mut rng));
            if rng.one_in(2) {
                // Another language's block whose lines look like fields.
                text.extend(["```text".to_owned(), "q: not an item?".to_owned()]);
                text.push(format!("did: {}", value(&mut rng)));
                text.push("```".to_owned());
            }
            let (body, marker) = well_formed(&mut rng);
            let (open, close) = fences(&mut rng);
            text.push(open);
            text.extend(body);
            text.push(close);
            expected.push(marker);
        }
        let text = text.join("\n");

        let parsed = parse_markers(&text);

        assert_eq!(parsed, expected, "case {case}: {text:?}");
    }
}

#[test]
fn crlf_line_endings_parse_like_lf() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x3a_0004, case);
        let text = if rng.one_in(2) {
            marker_soup(&mut rng)
        } else {
            let (body, _) = well_formed(&mut rng);
            let (open, close) = fences(&mut rng);
            format!("{open}\n{}\n{close}", body.join("\n"))
        };
        let lf = text.replace('\r', "");
        let crlf = lf.replace('\n', "\r\n");

        assert_eq!(
            parse_markers(&crlf),
            parse_markers(&lf),
            "case {case}: {lf:?}"
        );
    }
}

#[test]
fn trailing_whitespace_does_not_change_fields() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x3a_0005, case);
        let text = if rng.one_in(2) {
            marker_soup(&mut rng).replace('\r', "")
        } else {
            let (body, _) = well_formed(&mut rng);
            let (open, close) = fences(&mut rng);
            format!("{open}\n{}\n{close}", body.join("\n"))
        };
        let padded: Vec<String> = text
            .split('\n')
            .map(|line| format!("{line}{}", rng.pick(&["", " ", "\t", "  \t "])))
            .collect();
        let padded = padded.join("\n");

        let plain: Vec<Marker> = parse_markers(&text).into_iter().map(without_raw).collect();
        let spaced: Vec<Marker> = parse_markers(&padded)
            .into_iter()
            .map(without_raw)
            .collect();

        assert_eq!(spaced, plain, "case {case}: {text:?}");
    }
}
