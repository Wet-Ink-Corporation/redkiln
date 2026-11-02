//! The canonical JSON digest (SPEC ID-08): RFC 8785 (JCS).
//!
//! [`canonical_json`] writes a JSON value as RFC 8785 does: no whitespace, object keys sorted
//! by their UTF-16 code units, strings escaped as ECMAScript's `JSON.stringify` escapes them,
//! and numbers as ECMAScript's `Number.prototype.toString` writes the IEEE 754 double they are.
//! [`digest`] is the sha256 of that text, written `sha256:<64 hex>`. A payload's canonical
//! digest is taken over its `data` alone (ID-08, EV-13).
//!
//! # Numbers
//!
//! A float's digits are `serde_json`'s shortest round-trip digits (the ones ECMAScript chooses:
//! the fewest that read back as the same double, the closest of those to it), placed in
//! ECMAScript's notation here. RFC 8785 requires I-JSON input (RFC 7493), whose numbers are
//! doubles, so an integer beyond +-2^53, which a double may not hold exactly, is refused
//! ([`CanonicalError::Integer`]) rather than rounded to a different number.
//!
//! `serde_json`'s default parser reads some long decimal texts to a double one unit off the
//! nearest, so canonicalising parsed text inherits that; a value built in Rust is exact.

use std::cmp::Ordering;
use std::num::FpCategory;

use serde::Serialize;
use serde_json::{Number, Value};

use crate::ids::Digest;

/// The deepest nesting of arrays and objects that is canonicalised (H-13).
pub const MAX_DEPTH: usize = 128;

/// The largest integer magnitude every double holds exactly: 2^53.
const MAX_EXACT_INTEGER: u64 = 1 << 53;

/// Why a value has no canonical form.
#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    /// An integer beyond +-2^53, outside I-JSON, which RFC 8785 requires.
    #[error("the integer {text} is beyond +-2^53, which RFC 8785's I-JSON input excludes")]
    Integer {
        /// The integer as written.
        text: String,
    },
    /// A number whose `serde_json` text this module does not read, which no finite double has.
    #[error("the number {text} has no ECMAScript form here")]
    Number {
        /// The number as `serde_json` wrote it.
        text: String,
    },
    /// Arrays and objects nested deeper than [`MAX_DEPTH`].
    #[error("the value nests deeper than {MAX_DEPTH} levels")]
    TooDeep,
    /// The value could not be turned into JSON.
    #[error("serialising the value to JSON")]
    Serialize(#[source] serde_json::Error),
}

/// `value` in RFC 8785's canonical form.
///
/// # Errors
/// [`CanonicalError::Integer`] for an integer beyond +-2^53; [`CanonicalError::Number`] for a
/// number whose `serde_json` text has no ECMAScript form here (no finite double has one);
/// [`CanonicalError::TooDeep`] for nesting past [`MAX_DEPTH`].
pub fn canonical_json(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write_value(&mut out, value, 0)?;
    Ok(out)
}

/// The canonical digest of `value`: `sha256:` and the sha256 of [`canonical_json`] (ID-08).
///
/// # Errors
/// As [`canonical_json`].
pub fn digest(value: &Value) -> Result<Digest, CanonicalError> {
    Ok(Digest::sha256(canonical_json(value)?.as_bytes()))
}

/// The canonical digest of anything serde serialises, through its JSON value.
///
/// # Errors
/// [`CanonicalError::Serialize`] when `serde_json` refuses it, then as [`canonical_json`].
pub fn digest_of<T: Serialize + ?Sized>(value: &T) -> Result<Digest, CanonicalError> {
    digest(&serde_json::to_value(value).map_err(CanonicalError::Serialize)?)
}

fn write_value(out: &mut String, value: &Value, depth: usize) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(out, n)?,
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            let inner = nested(depth)?;
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item, inner)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let inner = nested(depth)?;
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|(a, _), (b, _)| utf16_order(a, b));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_value(out, item, inner)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// The depth inside a container at `depth`, refused past [`MAX_DEPTH`].
fn nested(depth: usize) -> Result<usize, CanonicalError> {
    let inner = depth.saturating_add(1);
    if inner > MAX_DEPTH {
        Err(CanonicalError::TooDeep)
    } else {
        Ok(inner)
    }
}

/// RFC 8785 section 3.2.3: keys compare as arrays of UTF-16 code units.
fn utf16_order(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// ECMAScript's `JSON.stringify` escapes: `"` and `\`, the five short control escapes, and
/// `\u00xx` (lowercase) for the other controls below U+0020; everything else as itself.
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c => match u8::try_from(c) {
                Ok(control) if control < 0x20 => {
                    out.push_str("\\u00");
                    out.push_str(&crate::ids::to_hex(&[control]));
                }
                _ => out.push(c),
            },
        }
    }
    out.push('"');
}

fn write_number(out: &mut String, n: &Number) -> Result<(), CanonicalError> {
    let integer = match (n.as_i64(), n.as_u64()) {
        (Some(i), _) => Some(i.unsigned_abs()),
        (None, Some(u)) => Some(u),
        (None, None) => None,
    };
    match (integer, n.as_f64()) {
        // An integer within 2^53 is a double whose ECMAScript form is its decimal digits.
        (Some(magnitude), _) if magnitude <= MAX_EXACT_INTEGER => out.push_str(&n.to_string()),
        (Some(_), _) => {
            return Err(CanonicalError::Integer {
                text: n.to_string(),
            });
        }
        (None, Some(f)) => write_double(out, f, &n.to_string())?,
        // Without serde_json's arbitrary precision every number is one of the three.
        (None, None) => {
            return Err(CanonicalError::Number {
                text: n.to_string(),
            });
        }
    }
    Ok(())
}

/// ECMAScript's `Number::toString(f)`, from `shortest`, `serde_json`'s shortest round-trip text of
/// the same double (`1.5e-7`, `123.0`, `-0.5`).
fn write_double(out: &mut String, f: f64, shortest: &str) -> Result<(), CanonicalError> {
    if f.classify() == FpCategory::Zero {
        // Negative zero too.
        out.push('0');
        return Ok(());
    }
    let unreadable = || CanonicalError::Number {
        text: shortest.to_owned(),
    };
    let (digits, n) = decimal_parts(shortest.trim_start_matches('-')).ok_or_else(unreadable)?;
    if f.is_sign_negative() {
        out.push('-');
    }
    write_ecmascript(out, &digits, n).ok_or_else(unreadable)
}

/// The significant digits `s` of a decimal text (`123.45`, `1.5e-7`) and the exponent `n` with
/// value `0.s * 10^n`, as ECMAScript's algorithm names them. `None` for text that is not a
/// positive decimal with significant digits.
fn decimal_parts(text: &str) -> Option<(String, i64)> {
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let all: String = int.chars().chain(frac.chars()).collect();
    if !all.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let leading = i64::try_from(all.chars().take_while(|&c| c == '0').count()).ok()?;
    let digits = all.trim_start_matches('0').trim_end_matches('0');
    if digits.is_empty() {
        return None;
    }
    let n = i64::try_from(int.len())
        .ok()?
        .checked_add(exponent)?
        .checked_sub(leading)?;
    Some((digits.to_owned(), n))
}

/// ECMA-262 `Number::toString` steps 7 to 10 for a positive value `0.digits * 10^n`, with `k`
/// the number of digits. `None` only when the parts are out of any double's range.
fn write_ecmascript(out: &mut String, digits: &str, n: i64) -> Option<()> {
    let k = i64::try_from(digits.len()).ok()?;
    if k <= n && n <= 21 {
        out.push_str(digits);
        out.extend((k..n).map(|_| '0'));
    } else if 0 < n && n <= 21 {
        let (whole, fraction) = digits.split_at_checked(usize::try_from(n).ok()?)?;
        out.push_str(whole);
        out.push('.');
        out.push_str(fraction);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend((n..0).map(|_| '0'));
        out.push_str(digits);
    } else {
        let (first, rest) = digits.split_at_checked(1)?;
        out.push_str(first);
        if !rest.is_empty() {
            out.push('.');
            out.push_str(rest);
        }
        let exponent = n.checked_sub(1)?;
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn number(bits: u64) -> String {
        let value = Number::from_f64(f64::from_bits(bits)).expect("finite");
        canonical_json(&Value::Number(value)).expect("canonical")
    }

    /// RFC 8785 Appendix B: IEEE 754 bit patterns and their ECMAScript serialisations.
    #[test]
    fn numbers_follow_rfc_8785_appendix_b() {
        let cases: [(u64, &str); 23] = [
            (0x0000_0000_0000_0000, "0"),
            (0x8000_0000_0000_0000, "0"),
            (0x0000_0000_0000_0001, "5e-324"),
            (0x8000_0000_0000_0001, "-5e-324"),
            (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
            (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
            (0x4340_0000_0000_0000, "9007199254740992"),
            (0xc340_0000_0000_0000, "-9007199254740992"),
            (0x4430_0000_0000_0000, "295147905179352830000"),
            (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
            (0x44b5_2d02_c7e1_4af6, "1e+23"),
            (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
            (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
            (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
            (0x444b_1ae4_d6e2_ef50, "1e+21"),
            (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
            (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
            (0x41b3_de43_5555_5553, "333333333.3333332"),
            (0x41b3_de43_5555_5554, "333333333.33333325"),
            (0x41b3_de43_5555_5555, "333333333.3333333"),
            (0x41b3_de43_5555_5556, "333333333.3333334"),
            (0x41b3_de43_5555_5557, "333333333.33333343"),
            (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        ];
        for (bits, expected) in cases {
            assert_eq!(number(bits), expected, "{bits:#018x}");
        }
        assert_eq!(number(0x4314_3ff3_c1cb_0959), "1424953923781206.2");
    }

    #[test]
    fn small_and_integral_floats_take_ecmascript_forms() {
        for (value, expected) in [
            (1.0, "1"),
            (4.5, "4.5"),
            (0.002, "0.002"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (1e30, "1e+30"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123.456, "123.456"),
            (-0.5, "-0.5"),
        ] {
            assert_eq!(
                canonical_json(&json!(value)).expect("canonical"),
                expected,
                "{value}"
            );
        }
    }

    /// RFC 8785 section 3.2.4's sample, with its numbers built from the values it names.
    #[test]
    fn the_rfc_sample_object_canonicalises_as_published() {
        let value = json!({
            "numbers": [333_333_333.333_333_3_f64, 1e30, 4.50, 2e-3, 1e-27],
            "string": "\u{20ac}$\u{f}\nA'B\"\\\\\"/",
            "literals": [null, true, false]
        });
        assert_eq!(
            canonical_json(&value).expect("canonical"),
            "{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"\u{20ac}$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}"
        );
    }

    /// RFC 8785 section 3.2.3: keys sort by UTF-16 code units, so the emoji (a surrogate pair,
    /// D83D) sorts before U+FB33, unlike a sort by code point.
    #[test]
    fn keys_sort_by_utf_16_code_units() {
        let value = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\u{1f600}": "Emoji: Grinning Face",
            "\u{80}": "Control",
            "\u{f6}": "Latin Small Letter O With Diaeresis"
        });
        let text = canonical_json(&value).expect("canonical");
        let order: Vec<&str> = [
            "\\r",
            "1",
            "\u{80}",
            "\u{f6}",
            "\u{20ac}",
            "\u{1f600}",
            "\u{fb33}",
        ]
        .into_iter()
        .collect();
        let mut last = 0;
        for key in order {
            let at = text.find(&format!("\"{key}\":")).expect(key);
            assert!(at >= last, "{key:?} out of order in {text}");
            last = at;
        }
    }

    #[test]
    fn strings_escape_only_what_ecmascript_escapes() {
        let value = json!("\u{8}\t\n\u{c}\r\u{1}\u{1f}\u{7f}\u{2028}/\"\\");
        assert_eq!(
            canonical_json(&value).expect("canonical"),
            "\"\\b\\t\\n\\f\\r\\u0001\\u001f\u{7f}\u{2028}/\\\"\\\\\""
        );
    }

    #[test]
    fn integers_within_two_to_the_53_are_written_whole() {
        let max = 9_007_199_254_740_992_i64;
        assert_eq!(
            canonical_json(&json!(max)).expect("2^53"),
            "9007199254740992"
        );
        assert_eq!(
            canonical_json(&json!(-max)).expect("-2^53"),
            "-9007199254740992"
        );
        assert_eq!(canonical_json(&json!(0)).expect("zero"), "0");
    }

    #[test]
    fn integers_beyond_two_to_the_53_are_refused_not_rounded() {
        assert!(matches!(
            canonical_json(&json!(9_007_199_254_740_993_u64)),
            Err(CanonicalError::Integer { .. })
        ));
        assert!(matches!(
            canonical_json(&json!(-9_007_199_254_740_993_i64)),
            Err(CanonicalError::Integer { .. })
        ));
        assert!(matches!(
            canonical_json(&json!(u64::MAX)),
            Err(CanonicalError::Integer { .. })
        ));
    }

    #[test]
    fn nesting_past_the_cap_is_refused() {
        let mut value = json!(1);
        for _ in 0..MAX_DEPTH {
            value = json!([value]);
        }
        let kept = canonical_json(&value).expect("exactly the cap is kept");
        assert_eq!(
            kept,
            format!("{}1{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH))
        );
        let deeper = json!([value]);
        assert!(matches!(
            canonical_json(&deeper),
            Err(CanonicalError::TooDeep)
        ));
    }

    #[test]
    fn the_digest_is_sha256_of_the_canonical_text() {
        let value = json!({"b": 1, "a": [true, null]});
        let digest = digest(&value).expect("digest");
        assert_eq!(digest, Digest::sha256(br#"{"a":[true,null],"b":1}"#));
        assert_eq!(
            digest_of(&value).expect("digest"),
            digest,
            "the same through serde"
        );
    }

    #[test]
    fn key_order_and_whitespace_do_not_change_the_digest() {
        let a: Value = serde_json::from_str(r#"{ "x": 1, "y": {"q": 2, "p": 3} }"#).expect("json");
        let b: Value = serde_json::from_str(r#"{"y":{"p":3,"q":2},"x":1}"#).expect("json");
        assert_eq!(digest(&a).expect("a"), digest(&b).expect("b"));
    }
}
