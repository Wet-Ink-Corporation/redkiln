//! Inline interpreter code (#117): the guard cannot read `python -c`, `node -e`, `perl -e` or
//! `ruby -e` code as commands, so one whose code or arguments name a config folder or a
//! protected file is denied. Code that builds a name at run time and a script file are limits.

use super::{ALLOW, CONFIG, assert_table};

#[test]
fn python_inline_code_naming_config_is_a_config_write() {
    assert_table(&[
        (
            "python3 -c \"open('.claude/settings.json', 'w').write('{}')\"",
            CONFIG,
        ),
        (
            "python3 -c 'import shutil; shutil.copytree(\"x\", \".redkiln\")'",
            CONFIG,
        ),
        (
            "python -c 'import os; os.remove(\".git/hooks/pre-commit\")'",
            CONFIG,
        ),
        ("python3 -c 'open(\"clippy.toml\", \"w\")'", CONFIG),
        (
            "python3 -c 'open(\"./.REDKILN/bench/config.json\", \"w\")'",
            CONFIG,
        ),
        (
            "python3 -c 'import sys, shutil; shutil.copy(*sys.argv[1:])' x .redkiln/bench",
            CONFIG,
        ),
        (
            "python3 -c 'import sys; open(sys.argv[1], \"w\")' /work/proj/.cargo/config.toml",
            CONFIG,
        ),
        ("python3 -Ic 'open(\".github/x\", \"w\")'", CONFIG),
        (
            "python3 -W ignore -c 'open(\".anneal/config.json\", \"w\")'",
            CONFIG,
        ),
        (
            "/usr/bin/python3.12 -c 'open(\".claude/x\", \"w\")'",
            CONFIG,
        ),
        ("python3 -m zipfile -e x.zip .redkiln", CONFIG),
        ("python3 -m tarfile -e x.tar .claude", CONFIG),
        (
            "bash -c \"python3 -c 'open(\\\".claude/x\\\", \\\"w\\\")'\"",
            CONFIG,
        ),
    ]);
}

#[test]
fn node_perl_and_ruby_inline_code_naming_config_is_a_config_write() {
    assert_table(&[
        (
            "node -e \"require('fs').writeFileSync('.claude/settings.json', '{}')\"",
            CONFIG,
        ),
        (
            "node --eval=\"fs.rmSync('.redkiln', {recursive: true})\"",
            CONFIG,
        ),
        (
            "node --eval \"fs.cpSync('x', '.github', {recursive: true})\"",
            CONFIG,
        ),
        ("node -p \"fs.writeFileSync('deny.toml', '')\"", CONFIG),
        ("nodejs -r fs -e \"x('.cargo/config.toml')\"", CONFIG),
        ("perl -pi -e 's/a/b/' .claude/settings.json", CONFIG),
        (
            "perl -i.bak -pe 's/a/b/' .redkiln/bench/config.json",
            CONFIG,
        ),
        ("perl -E 'open(F, \">.anneal/config.json\")'", CONFIG),
        // `-I` takes the next word (review F1).
        ("perl -I lib -e 'open(F, \">.claude/x\")'", CONFIG),
        // An unlisted long option may take the next word: the scan reads on (review F2).
        (
            "node --disable-warning X -e \"fs.writeFileSync('.claude/x', '')\"",
            CONFIG,
        ),
        (
            "python3 --unlisted X -c 'open(\".claude/x\", \"w\")'",
            CONFIG,
        ),
        ("perl -lne 'print' rustfmt.toml", CONFIG),
        (
            "ruby -e 'File.write(\".claude/settings.json\", \"{}\")'",
            CONFIG,
        ),
        ("ruby -i -pe 'gsub(/a/, \"b\")' lefthook.yml", CONFIG),
    ]);
}

#[test]
fn inline_code_naming_no_config_and_script_files_are_left_to_the_fallback() {
    assert_table(&[
        ("python3 -c 'print(1)'", ALLOW),
        ("python3 -c 'print(\"webhooks\")'", ALLOW),
        ("python3 -c 'print(\"my.claude\")'", ALLOW),
        ("python3 -c 'import json' build/out.json", ALLOW),
        ("python3 script.py .redkiln", ALLOW),
        ("python3 script.py -c '.claude'", ALLOW),
        ("python3 -m pytest tests", ALLOW),
        ("node -e 'console.log(1)'", ALLOW),
        ("node app.js -e .claude", ALLOW),
        ("perl -ne 'print' notes.md", ALLOW),
        ("perl script.pl .claude", ALLOW),
        ("ruby -e 'puts 1'", ALLOW),
        // A dotless name such as `hooks` counts only beside a path separator (review F5).
        ("python3 -c 'import hooks'", ALLOW),
        ("python3 -c 'hooks = 1; print(hooks)'", ALLOW),
        ("pytest -k hooks", ALLOW),
        ("python3 -c \"open('git/hooks/pre-commit', 'w')\"", CONFIG),
        ("python3 -c \"open('hooks/x', 'w')\"", CONFIG),
        ("ruby -E utf-8 script.rb .claude", ALLOW),
        ("python3 -c 'print(1)' ..", ALLOW),
    ]);
}
