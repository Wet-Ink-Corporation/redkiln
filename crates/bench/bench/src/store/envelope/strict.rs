//! The JSON of the envelope `bench ledger append` takes, read with no key named twice in any
//! object (#135, `kb-decision-e09e24b848`).
//!
//! `serde_json` reads a repeated key last-wins, so `{"task":"T1","task":"T2"}` would append
//! `T2` where a reader keeping the first sees `T1`. Core already refuses a stored envelope that
//! repeats a key (EV-10); this reads `append`'s input the same way, at every depth, and names
//! the repeated key by its path from the envelope's root.

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::map::Entry;
use serde_json::{Map, Number, Value};

use super::SchemaError;

/// `text` as JSON, each object naming each key once.
///
/// # Errors
/// [`SchemaError::RepeatedKey`] naming the first repeated key's path (`data.task`, an array
/// item as `[0]`); [`SchemaError::Json`] when `text` is not JSON.
pub(super) fn parse(text: &str) -> Result<Value, SchemaError> {
    let mut repeated = None;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let read = Seed {
        at: None,
        repeated: &mut repeated,
    }
    .deserialize(&mut deserializer)
    .and_then(|value| deserializer.end().map(|()| value));
    match (read, repeated) {
        (Err(_repeat), Some(field)) => Err(SchemaError::RepeatedKey { field }),
        (read, _) => read.map_err(SchemaError::Json),
    }
}

/// One step of a path from the envelope's root: a key or an array index.
#[derive(Clone, Copy)]
enum Step<'a> {
    /// A key of an object.
    Key(&'a str),
    /// An index into an array.
    Index(usize),
}

/// Where a value sits: its last step and the place of what holds it.
struct At<'a> {
    parent: Option<&'a At<'a>>,
    step: Step<'a>,
}

/// The dotted path of `key` inside the value at `at`.
fn path(at: Option<&At<'_>>, key: &str) -> String {
    let mut steps = vec![Step::Key(key)];
    let mut next = at;
    while let Some(place) = next {
        steps.push(place.step);
        next = place.parent;
    }
    let mut text = String::new();
    for (depth, step) in steps.iter().rev().enumerate() {
        match step {
            Step::Key(name) if depth == 0 => text.push_str(name),
            Step::Key(name) => {
                text.push('.');
                text.push_str(name);
            }
            Step::Index(index) => {
                text.push('[');
                text.push_str(&index.to_string());
                text.push(']');
            }
        }
    }
    text
}

/// Reads one value at `at`, recording a repeated key's path in `repeated` before failing.
struct Seed<'a, 'r> {
    at: Option<&'a At<'a>>,
    repeated: &'r mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for Seed<'_, '_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_, '_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with no repeated key")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        // JSON text never holds a non-finite number, so this refuses nothing serde_json reads.
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("a non-finite number"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        loop {
            let at = At {
                parent: self.at,
                step: Step::Index(items.len()),
            };
            let seed = Seed {
                at: Some(&at),
                repeated: &mut *self.repeated,
            };
            match seq.next_element_seed(seed)? {
                Some(item) => items.push(item),
                None => return Ok(Value::Array(items)),
            }
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            match object.entry(key) {
                Entry::Occupied(seen) => {
                    let field = path(self.at, seen.key());
                    let error = de::Error::custom(format_args!("repeated key {field:?}"));
                    *self.repeated = Some(field);
                    return Err(error);
                }
                Entry::Vacant(slot) => {
                    let at = At {
                        parent: self.at,
                        step: Step::Key(slot.key()),
                    };
                    let value = map.next_value_seed(Seed {
                        at: Some(&at),
                        repeated: &mut *self.repeated,
                    })?;
                    slot.insert(value);
                }
            }
        }
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
mod tests {
    use super::{SchemaError, parse};
    use serde_json::Value;

    #[test]
    fn a_text_with_no_repeated_key_reads_as_serde_json_reads_it() {
        let text = r#"{"a":[1,-2,3.5,true,null,"x",{"b":{}}],"c":18446744073709551615}"#;
        let plain: Value = serde_json::from_str(text).expect("json");

        let read = parse(text).expect("no repeated key");

        assert_eq!(read, plain, "the same value");
    }

    #[test]
    fn a_repeated_key_is_refused_by_its_path_at_any_depth() {
        for (text, expected) in [
            (
                r#"{"type":"t","data":{"task":"T1","task":"T2"}}"#,
                "data.task",
            ),
            (
                r#"{"data":{"tokens":{"input":1,"input":2}}}"#,
                "data.tokens.input",
            ),
            (
                r#"{"data":{"items":[{"a":1},{"b":1,"b":2}]}}"#,
                "data.items[1].b",
            ),
            (r#"{"type":"a","type":"b","data":{}}"#, "type"),
        ] {
            let read = parse(text);

            assert!(
                matches!(&read, Err(SchemaError::RepeatedKey { field }) if field == expected),
                "{text}: {read:?}"
            );
        }
    }

    #[test]
    fn text_that_is_not_json_stays_a_json_error() {
        for text in ["not json", r#"{"a":1} trailing"#, r#"{"a":"#] {
            let read = parse(text);

            assert!(
                matches!(read, Err(SchemaError::Json(_))),
                "{text}: {read:?}"
            );
        }
    }
}
