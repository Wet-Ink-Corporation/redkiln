//! The SPEC's CLI envelope (CLI-01 to CLI-04) checked on what the binary writes: every envelope a
//! test reads through [`envelope`] passes `spec/schemas/cli-envelope-1.schema.json`, and
//! [`golden`] pins one envelope per output id under `tests/fixtures/cli/`, normalised so paths,
//! hashes, commit ids and times do not vary between runs. `BENCH_BLESS=1` (or anneal's
//! `ANNEAL_BLESS=1`) rewrites the goldens a run reaches.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Output;

use bench_contract::cli::OUTPUTS;
use serde_json::{Map, Value};

/// A test's result, with any setup error.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The SPEC schema `spec/schemas/<stem>.schema.json`.
fn schema(stem: &str) -> TestResult<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/schemas")
        .join(format!("{stem}.schema.json"));
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// Every way `doc` breaks the SPEC schema `stem`, as `<instance path>: <error>`.
fn breaks(stem: &str, doc: &Value) -> TestResult<Vec<String>> {
    let validator = jsonschema::draft202012::new(&schema(stem)?)?;
    Ok(validator
        .iter_errors(doc)
        .map(|e| format!("{}: {e}", e.instance_path().as_str()))
        .collect())
}

/// Every way `envelope` breaks the CLI envelope schema, as `<instance path>: <error>`.
pub(crate) fn schema_errors(envelope: &Value) -> TestResult<Vec<String>> {
    breaks("cli-envelope-1", envelope)
}

/// Every way `data` breaks CLI-12's `status-1` schema, as `<instance path>: <error>`.
pub(crate) fn status_errors(data: &Value) -> TestResult<Vec<String>> {
    breaks("status-1", data)
}

/// `output`'s stdout as the one envelope CLI-01 allows: a single line ended by `\n`, `schema`
/// its first key and one of `bench_contract::cli::OUTPUTS`, passing the envelope schema, with
/// `ok` true exactly when the exit code is 0 (CLI-02), and a `bench.status/1` envelope's `data`
/// passing `status-1` (CLI-12). Panics, naming what broke, otherwise.
pub(crate) fn envelope(output: &Output) -> TestResult<Value> {
    let text = std::str::from_utf8(&output.stdout)?;
    assert!(
        text.ends_with('\n') && text.matches('\n').count() == 1,
        "CLI-01: one line ended by a newline: {output:?}"
    );
    assert!(
        text.starts_with(r#"{"schema":"bench."#),
        "CLI-02: schema is the first key: {text}"
    );
    let envelope: Value = serde_json::from_str(text)?;
    let errors = schema_errors(&envelope)?;
    assert!(
        errors.is_empty(),
        "{text} breaks cli-envelope-1: {errors:?}"
    );
    let id = envelope["schema"].as_str().unwrap_or_default();
    assert!(
        OUTPUTS.contains(&id),
        "{id} is in bench_contract::cli::OUTPUTS"
    );
    assert_eq!(
        envelope["ok"].as_bool(),
        Some(output.status.code() == Some(0)),
        "CLI-02: ok agrees with the exit code: {output:?}"
    );
    if id == "bench.status/1" {
        let errors = status_errors(&envelope["data"])?;
        assert!(errors.is_empty(), "{text} breaks status-1: {errors:?}");
    }
    Ok(envelope)
}

/// Where the golden of output `id` lives: `tests/fixtures/cli/<id>.json`, the `/<n>` written
/// `.v<n>` (`bench.dag-check/1` is `bench.dag-check.v1.json`).
pub(crate) fn golden_path(id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cli")
        .join(format!("{}.json", id.replace('/', ".v")))
}

/// Whether this run rewrites goldens: `BENCH_BLESS`, or anneal's `ANNEAL_BLESS`, set. It
/// mirrors `src/bless.rs`'s `blessing`, which is crate-private and so out of this test
/// binary's reach; change both together.
fn blessing() -> bool {
    std::env::var_os("BENCH_BLESS").is_some() || std::env::var_os("ANNEAL_BLESS").is_some()
}

/// Checks `envelope` against the golden of its own output id (CLI-04), once normalised: every
/// string with each of `roots` (as given and canonical) written `<root>`, `sha256:` digests
/// `sha256:<hash>`, other 64- and 40-digit hex runs `<hash>` and `<sha>`, RFC 3339 times
/// `<time>`, and the values at each pointer in `varies` (a `*` segment matches any key or
/// index) `"<varies>"`. Writes the golden instead when blessing.
///
/// `id` is the output id the caller expects, written as a literal at the call site so
/// `cli_envelope::every_output_id_is_compared_by_a_golden_call` can see every id is compared;
/// the envelope must be that id's.
pub(crate) fn golden(id: &str, envelope: &Value, roots: &[&Path], varies: &[&str]) -> TestResult {
    assert_eq!(
        envelope["schema"].as_str(),
        Some(id),
        "the golden compared is the id the call names: {envelope}"
    );
    let mut prefixes: Vec<String> = Vec::new();
    for root in roots {
        let mut spellings = vec![root.to_string_lossy().into_owned()];
        if let Ok(canonical) = std::fs::canonicalize(root) {
            spellings.push(canonical.to_string_lossy().into_owned());
        }
        for spelling in spellings {
            prefixes.extend(root_spellings(&spelling));
        }
    }
    // Longest first, so a canonical path is not cut by a shorter prefix of it.
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let mut normal = normalise(envelope, &prefixes);
    for pointer in varies {
        let segments: Vec<&str> = pointer.split('/').skip(1).collect();
        assert!(
            mark_varies(&mut normal, &segments),
            "{id}: {pointer} names a value in {envelope}"
        );
    }
    let path = golden_path(id);
    if blessing() {
        std::fs::create_dir_all(path.parent().ok_or("a fixtures directory")?)?;
        std::fs::write(&path, format!("{}\n", in_wire_order(&normal)?))?;
    }
    let committed: Value =
        serde_json::from_slice(&std::fs::read(&path).map_err(|error| {
            format!("{}: {error}; write it with BENCH_BLESS=1", path.display())
        })?)?;
    assert_eq!(
        normal,
        committed,
        "{id} drifted from {}; if the change is meant, regenerate it with BENCH_BLESS=1 and \
         review the diff",
        path.display()
    );
    Ok(())
}

/// The ways the binary may write a root it was given as `path`: as given, with `/` for every
/// `\` (Bench writes Windows paths with forward slashes), and on Windows without the `\\?\`
/// verbatim prefix `canonicalize` adds, again in both slash forms.
fn root_spellings(path: &str) -> Vec<String> {
    let mut spellings = vec![path.to_owned(), path.replace('\\', "/")];
    if let Some(plain) = path.strip_prefix(r"\\?\") {
        spellings.push(plain.to_owned());
        spellings.push(plain.replace('\\', "/"));
    }
    spellings
}

/// The envelope's keys in CLI-02's order, so a golden reads as the binary writes it.
#[derive(serde::Serialize)]
struct WireOrder<'a> {
    schema: &'a Value,
    ok: &'a Value,
    command: &'a Value,
    data: &'a Value,
    findings: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    proves: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    does_not_prove: Option<&'a Value>,
}

/// `envelope` pretty-printed with its top-level keys in CLI-02's order.
fn in_wire_order(envelope: &Value) -> TestResult<String> {
    let wire = WireOrder {
        schema: &envelope["schema"],
        ok: &envelope["ok"],
        command: &envelope["command"],
        data: &envelope["data"],
        findings: &envelope["findings"],
        proves: envelope.get("proves"),
        does_not_prove: envelope.get("does_not_prove"),
    };
    Ok(serde_json::to_string_pretty(&wire)?)
}

/// `value` with every string and key normalised (see [`golden`]).
fn normalise(value: &Value, prefixes: &[String]) -> Value {
    match value {
        Value::String(text) => Value::String(normal_text(text, prefixes)),
        Value::Array(items) => Value::Array(items.iter().map(|v| normalise(v, prefixes)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (normal_text(k, prefixes), normalise(v, prefixes)))
                .collect::<Map<_, _>>(),
        ),
        other => other.clone(),
    }
}

/// `text` with roots, digests, commit ids and times replaced (see [`golden`]).
fn normal_text(text: &str, prefixes: &[String]) -> String {
    let mut text = text.to_owned();
    for prefix in prefixes.iter().filter(|p| !p.is_empty()) {
        text = text.replace(prefix.as_str(), "<root>");
    }
    let text = text.replace('\\', "/");
    times(&hex_runs(&text))
}

/// `text` with each maximal run of lowercase hex digits of length 64 written `<hash>` and of
/// length 40 written `<sha>`.
fn hex_runs(text: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        match run.len() {
            64 => out.push_str("<hash>"),
            40 => out.push_str("<sha>"),
            _ => out.push_str(run),
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() || ('a'..='f').contains(&c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// `text` with each RFC 3339 time (`YYYY-MM-DDTHH:MM:SS`, optional fraction, `Z` or an offset)
/// written `<time>`.
fn times(text: &str) -> String {
    const SHAPE: &[u8] = b"dddd-dd-ddTdd:dd:dd";
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let fits = bytes.get(i..i + SHAPE.len()).is_some_and(|window| {
            window.iter().zip(SHAPE).all(|(b, s)| match s {
                b'd' => b.is_ascii_digit(),
                other => b == other,
            })
        });
        if !fits {
            let Some(c) = text.get(i..).and_then(|rest| rest.chars().next()) else {
                break;
            };
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let mut end = i + SHAPE.len();
        if bytes.get(end) == Some(&b'.') {
            end += 1;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
        }
        match bytes.get(end) {
            Some(b'Z') => end += 1,
            Some(b'+' | b'-') if bytes.get(end + 3) == Some(&b':') => end += 6,
            _ => {}
        }
        out.push_str("<time>");
        i = end.min(bytes.len());
    }
    out
}

/// Replaces the values `segments` names in `value` (`*` any key or index) by `"<varies>"`;
/// whether any was found.
fn mark_varies(value: &mut Value, segments: &[&str]) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        *value = Value::String("<varies>".to_owned());
        return true;
    };
    match value {
        // `|`, not `any`: every branch is marked, not only the first that matches.
        Value::Object(map) if *first == "*" => map
            .values_mut()
            .fold(false, |found, v| found | mark_varies(v, rest)),
        Value::Object(map) => map.get_mut(*first).is_some_and(|v| mark_varies(v, rest)),
        Value::Array(items) if *first == "*" => items
            .iter_mut()
            .fold(false, |found, v| found | mark_varies(v, rest)),
        Value::Array(items) => first
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get_mut(index))
            .is_some_and(|v| mark_varies(v, rest)),
        _ => false,
    }
}

#[test]
fn normalising_replaces_roots_digests_commits_and_times() {
    let prefixes = ["/tmp/x".to_owned()];
    assert_eq!(
        normal_text(
            &format!(
                "/tmp/x/a at {} sha256:{} 2026-11-11T10:00:00.5Z",
                "a".repeat(40),
                "0".repeat(64)
            ),
            &prefixes
        ),
        "<root>/a at <sha> sha256:<hash> <time>",
        "each kind of varying text"
    );
}

#[test]
fn a_windows_root_is_found_in_its_forward_slash_and_unverbatim_spellings() {
    let prefixes = root_spellings(r"\\?\C:\Users\me\Temp\.tmpA");
    assert_eq!(
        normal_text(
            "C:/Users/me/Temp/.tmpA/.git/redkiln/events.sqlite3",
            &prefixes
        ),
        "<root>/.git/redkiln/events.sqlite3",
        "the binary writes the canonical root with forward slashes and no verbatim prefix"
    );
}

#[test]
fn a_varies_pointer_matches_every_index_under_a_wildcard() {
    let mut value = serde_json::json!({"runs": [{"ms": 1}, {"ms": 2}]});
    assert!(mark_varies(&mut value, &["runs", "*", "ms"]), "found");
    assert_eq!(
        value,
        serde_json::json!({"runs": [{"ms": "<varies>"}, {"ms": "<varies>"}]}),
        "both replaced"
    );
}
