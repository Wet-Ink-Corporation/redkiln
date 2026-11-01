//! The source write-back of `decide`, end to end: a line of prose (markdown or plain text) gets
//! the arrow; a code file is left byte for byte as it was, and the decision lives in the ledger,
//! the atom and the trailer.

use crate::support::{decide_leaves_code_alone, scan_one, setup, stamped_date, weighin};
use serde_json::json;

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_markdown_question_is_annotated_at_the_end_of_its_line() {
    let (_tmp, repo) = setup();
    let source = "# Plan\n\n## Open questions\n- Should we shard by tenant?\n- tail\n";
    std::fs::write(repo.join("docs/q.md"), source).unwrap();
    let id = scan_one(&repo, "docs/q.md");

    let (ok, out) = weighin(&repo, "s1", &["decide", &id, "no"], None);

    assert!(ok && out.contains("annotated docs/q.md"), "{out}");
    let after = std::fs::read_to_string(repo.join("docs/q.md")).unwrap();
    let today = stamped_date(&after, &id, "no");
    assert_eq!(
        after,
        format!(
            "# Plan\n\n## Open questions\n- Should we shard by tenant? → Weigh-In {id}: decided no ({today})\n- tail\n"
        )
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_question_quoted_from_a_text_file_is_annotated_at_the_end_of_its_line() {
    let (_tmp, repo) = setup();
    std::fs::write(repo.join("notes.txt"), "Should the cache be shared?\n").unwrap();
    let raise = json!([{"op": "raise", "q": "Should the cache be shared?", "door": "two-way",
        "authority": "judgment", "channel": "sweep", "quote": "Should the cache be shared?",
        "source_path": "notes.txt", "source_line": 1}]);
    let (ok, out) = weighin(&repo, "s1", &["batch"], Some(&raise.to_string()));
    assert!(ok, "{out}");
    let id = out
        .lines()
        .find_map(|l| l.strip_prefix("✓ raised "))
        .and_then(|l| l.split_whitespace().next())
        .expect("the raise succeeded")
        .to_owned();

    let (ok, out) = weighin(&repo, "s1", &["decide", &id, "yes"], None);

    assert!(ok && out.contains("annotated notes.txt"), "{out}");
    let after = std::fs::read_to_string(repo.join("notes.txt")).unwrap();
    let today = stamped_date(&after, &id, "yes");
    assert_eq!(
        after,
        format!("Should the cache be shared? → Weigh-In {id}: decided yes ({today})\n")
    );
}

/// Code lines a decision must not touch: every line the five review rounds found (a triple
/// quote, a regex with a quote, a lifetime, a digit separator, a verbatim string, PHP's `?>`,
/// JSX text, byte strings, a declared coding), and the whole-line comments the old per-line rule
/// did annotate.
const LEFT_ALONE: &[(&str, &str)] = &[
    (
        "src/cap.py",
        "def f():\n    # TODO: pick a cap\n    return 1\n",
    ),
    ("src/x.c", "/* TODO: pick a cap */\n"),
    (
        "src/a.c",
        "int main(void) {\n    /* TODO: pick a buffer size */\n    return 0;\n}\n",
    ),
    ("src/c.rs", "fn f() {\n    // TODO: pick a cap\n}\n"),
    ("src/t.py", "x = 1  # TODO: pick a start\n"),
    ("src/f.c", "/* a */ int x; // TODO: pick a width\n"),
    (
        "src/crlf.rs",
        "fn f() {\r\n    // TODO: pick a cap\r\n}\r\n",
    ),
    ("src/d.rs", "    *limit = todo!(\"pick a cap\");\n"),
    (
        "src/half.py",
        "half = len(items) // 2 if use_temporary_cache else 0\n",
    ),
    (
        "src/get.sh",
        "curl -s https://jsonplaceholder.typicode.com/todo/items | jq .\n",
    ),
    ("src/a.py", "help = '''Don't use # TODO: explain'''\n"),
    ("src/a.js", "s.replace(/'/g, '// TODO: pick a separator')\n"),
    (
        "src/a.cs",
        "var p = @\"C:\\\"; Log(\"// TODO: x\" + '\"');\n",
    ),
    ("src/a.php", "// TODO: pick ?> <p>\n"),
    ("src/a.jsx", "  return <p>// TODO: x</p>;\n"),
    ("src/s.c", "#define X 1 // TODO: q9 word9 \\\n"),
    ("src/t.c", "// TODO: q9 word9 \\\n"),
    ("src/sep.js", "// TODO: pick \u{2028} let x=1;\n"),
    ("src/w.c", "/*\n// TODO: pick a width */\nint x;\n"),
    (
        "src/doc.py",
        "def f():\n    \"\"\"Doc.\n    # TODO: pick a cap\"\"\"\n",
    ),
    ("src/t.js", "const s = `\n// TODO: pick a tag`;\n"),
    ("src/e.sh", "echo 'a\n# TODO: pick a word'\n"),
    ("src/q.php", "<?php\n$s = 'a\n// TODO: pick a cap';\n"),
    ("src/h.php", "<?php\n$s = 'a\n# TODO: pick a cap';\n"),
    ("src/q.js", "const s = 'a\\\n// TODO: pick a cap';\n"),
    ("src/b.py", "data = b\"\"\"\n# TODO: pick a cap\n\"\"\"\n"),
    (
        "src/b.rs",
        "const S: &[u8] = br#\"\n// TODO: pick a cap\n\"#;\n",
    ),
    (
        "src/ascii.py",
        "# -*- coding: ascii -*-\n# TODO: pick a cap\n",
    ),
];

/// Whole-line comments inside a string opened on an earlier line, with an option that would
/// close or interpolate into that string (review: bash, PHP, Ruby and Perl). A code file is
/// never annotated, so none of these options reaches the file.
const SIGIL_OPTIONS: &[(&str, &str, &str)] = &[
    (
        "src/s.py",
        "s = \"\"\"\n# TODO: pick a cap\n\"\"\"\n",
        "x \"\"\" y",
    ),
    (
        "src/b.py",
        "s = \"\"\"\n# TODO: pick a cap\n\"\"\"\n",
        r"C:\N",
    ),
    ("src/e.sh", "echo \"a\n# TODO: pick a cap\nb\"\n", "$(nproc"),
    ("src/e2.sh", "echo \"a\n# TODO: pick a cap\nb\"\n", "$(("),
    (
        "src/h.php",
        "<?php\n$s = <<<EOT\n// TODO: pick a cap\nEOT;\n",
        "$a[",
    ),
    ("src/q.rb", "x = %q(\n# TODO: pick a cap\n)\n", "a)"),
    ("src/q.pl", "my $x = q(\n# TODO: pick a cap\n);\n", "a)"),
    (
        "src/h.pl",
        "use strict;\nmy $x = <<\"EOT\";\n# TODO: pick a cap\nEOT\n",
        "me@example",
    ),
    (
        "src/a.php",
        "<?php\n$s = '\n// TODO: pick a cap\n';\n",
        "don't",
    ),
    ("src/r.pl", "my $x = qr/a\n# TODO: pick a cap\n/x;\n", "a/b"),
];

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_lifetime_apostrophe_does_not_open_a_comment_so_its_line_raises_nothing() {
    let (_tmp, repo) = setup();
    let source = "fn f(x: impl Fn() + 'static) { panic!(\"TODO: pick a limit\") }\n";
    std::fs::write(repo.join("src/l.rs"), source).unwrap();

    let (ok, out) = weighin(&repo, "s1", &["scan", "src/l.rs"], None);

    assert!(ok, "{out}");
    assert_eq!(out.trim_end(), "nothing to raise");
    assert_eq!(
        std::fs::read(repo.join("src/l.rs")).unwrap(),
        source.as_bytes()
    );
}

/// Comment markers inside double-quoted strings, which earlier rounds raised and then had to leave
/// alone. Since wi-52e471 they are not raised at all.
const NOT_RAISED: &[(&str, &str)] = &[
    (
        "src/c.js",
        "const s = \"// TODO: decide later what this is\"; console.log(s);\n",
    ),
    (
        "src/a.rs",
        "let v: &'a str = pick(\"' // TODO: decide\", '\"');\n",
    ),
    (
        "src/a.cpp",
        "int x = 1'000; const char* s = \"' // TODO: q\"; char c = '\"';\n",
    ),
];

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_todo_inside_a_string_literal_is_not_raised() {
    for &(file, source) in NOT_RAISED {
        let (_tmp, repo) = setup();
        std::fs::write(repo.join(file), source).unwrap();

        let (ok, out) = weighin(&repo, "s1", &["scan", file], None);

        assert!(ok, "{file}: {out}");
        assert_eq!(out.trim_end(), "nothing to raise", "{file}");
    }
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_code_file_is_decided_but_left_byte_for_byte_unchanged() {
    for &(file, source) in LEFT_ALONE {
        decide_leaves_code_alone(file, source, "X");
    }
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_option_that_could_close_a_string_never_reaches_a_code_file() {
    for &(file, source, option) in SIGIL_OPTIONS {
        decide_leaves_code_alone(file, source, option);
    }
}
