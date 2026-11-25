//! Writers the guard did not read before #117: `rsync`, `tar` extracting and `unzip -d` whose
//! destination is a config folder or a protected file. Like `cp`, a config folder named as the
//! destination is denied because the guard cannot tell whether it exists, so the archive or
//! the source tree may bring its own config into it. The project is `/work/proj`.

use super::{ALLOW, CONFIG, assert_table};

#[test]
fn rsync_into_a_config_folder_or_onto_protected_config_is_a_config_write() {
    assert_table(&[
        ("rsync -a x/ .redkiln/", CONFIG),
        ("rsync -a x .redkiln", CONFIG),
        ("rsync -a x/ ./.redkiln", CONFIG),
        ("rsync -a x/ .redkiln/bench", CONFIG),
        ("rsync -a x/ .redkiln/bench/", CONFIG),
        ("rsync -av --delete x/ .claude/", CONFIG),
        ("rsync x.json .redkiln/bench/config.json", CONFIG),
        ("rsync -a x/ /work/proj/.redkiln/", CONFIG),
        ("rsync settings.json .claude", CONFIG),
        ("rsync -a /tmp/x/ .anneal", CONFIG),
        ("rsync -r /tmp/.github .", CONFIG),
        // Options may follow the operands.
        ("rsync x/ .github/ -a", CONFIG),
        ("rsync -a --exclude '*.tmp' x/ .cargo/", CONFIG),
        ("rsync -a -e ssh x/ .cargo", CONFIG),
        ("rsync -a -- x/ .redkiln/", CONFIG),
        // Value options after the operands must not leave their value as the destination
        // (review F3).
        ("rsync -a x/ .claude/ --outbuf L", CONFIG),
        ("rsync -a x/ .claude/ --stderr all", CONFIG),
        ("rsync -a x/ .claude/ --copy-as user", CONFIG),
        // Flags after the operands (#135): a trailing `--delete` and its kin take no value.
        ("rsync -a x/ .claude/ --delete", CONFIG),
        ("rsync x/ .redkiln/ --delete --delete-excluded -v", CONFIG),
        ("rsync x/ .github/ --delete --exclude tmp", CONFIG),
        // An unlisted long option after the destination may take a value: the operand before
        // its value is judged as the destination too, so the value cannot stand in for it
        // (#135, #130 review).
        ("rsync -a x/ .claude/ --unlisted-option L", CONFIG),
        ("rsync -a x/ .redkiln/ --delete --unlisted-option L", CONFIG),
        ("rsync -a x/ build/ --unlisted-option .claude", CONFIG),
        // An unlisted flag before the operands must not swallow a source when a later unlisted
        // option's value is weighed (#156 review F1).
        ("rsync -av --delete x/ .claude/ --unlisted-option L", CONFIG),
        (
            "rsync -a --progress x/ .claude/ --unlisted-option L",
            CONFIG,
        ),
        (
            "rsync -a x/ --progress .claude/ --unlisted-option L",
            CONFIG,
        ),
        // `--log-format` is the old name of `--out-format` and takes a value.
        ("rsync -a --delete x/ .claude/ --log-format L", CONFIG),
        // Over-denial, accepted (#156 review F3): `--progress` is a flag, so `/tmp/backup/` is
        // the destination, but read as taking a value it leaves `.github/` as one.
        (
            "rsync -av .claude/ .github/ --progress /tmp/backup/",
            CONFIG,
        ),
        ("cd .redkiln && rsync -a /tmp/x/ bench/", CONFIG),
        ("bash -c 'rsync -a x/ .redkiln/'", CONFIG),
        ("sudo rsync -a x/ .redkiln/", CONFIG),
        ("env LC_ALL=C nice tar -xf x.tar -C .claude", CONFIG),
        ("timeout 5 unzip x.zip -d .redkiln", CONFIG),
    ]);
}

#[test]
fn rsync_moving_config_away_is_a_config_write() {
    assert_table(&[
        (
            "rsync --remove-source-files .claude/settings.json /tmp/x",
            CONFIG,
        ),
        ("rsync -a --remove-source-files .redkiln /tmp/x", CONFIG),
        ("rsync -a .claude/settings.json /tmp/x", ALLOW),
    ]);
}

#[test]
fn rsync_elsewhere_is_left_to_the_fallback() {
    assert_table(&[
        ("rsync -a x/ .redkiln/bench/x", ALLOW),
        ("rsync -a x/ build/", ALLOW),
        ("rsync -a .redkiln/ /tmp/backup/", ALLOW),
        ("rsync -a .claude /tmp/backup", ALLOW),
        ("rsync -a x/ host:.redkiln/", ALLOW),
        ("rsync -a --exclude .claude x/ build/", ALLOW),
        ("rsync -a -e 'ssh -F .claude/x' x/ build/", ALLOW),
        ("rsync --list-only .redkiln", ALLOW),
        ("rsync -a x/ ..", ALLOW),
        ("rsync -a x/ build/ --delete", ALLOW),
        ("rsync -a .claude/ /tmp/backup/ --unlisted-option L", ALLOW),
    ]);
}

#[test]
fn rsync_option_looking_values_cannot_hide_the_destination() {
    // popt takes a value's next word whole, even one starting with `-`, so an unlisted long
    // option may take a listed value option as its value, leaving that option's own value as
    // the destination (#160).
    assert_table(&[
        ("rsync x/ L --newa --exclude .claude/", CONFIG),
        ("rsync x/ L --newa -e .claude/", CONFIG),
        ("rsync x/ L --newa --newb --exclude .claude/", CONFIG),
        ("rsync x/ L --newa --exclude .claude/ --newb M", CONFIG),
        ("rsync -a x/ L --newa --filter .redkiln/bench", CONFIG),
        // Read as `--newa` taking `--exclude`, the `--` ends the options, so `--filter` and
        // `.claude/` are operands.
        ("rsync x/ L --newa --exclude -- --filter .claude/", CONFIG),
        // In the reading that makes `--exclude` the value, `x/` is a source copied into
        // `.claude/` (review of #179, F1).
        ("rsync --newa --exclude x/ .claude/", CONFIG),
        // Over-denial, accepted: `--progress` is a flag, but read as taking `--exclude` it
        // leaves `.claude` a source copied to `build/.claude`.
        ("rsync --progress --exclude .claude x/ build/", CONFIG),
        (
            "rsync x/ .claude/settings.json --newa --exclude --remove-source-files /tmp/x",
            CONFIG,
        ),
    ]);
}

#[test]
fn rsync_option_looking_values_elsewhere_stay_allowed() {
    assert_table(&[
        // A word freed when an unlisted option takes the listed one is judged as a source or
        // a destination like any other operand.
        ("rsync --progress --exclude tmp x/ build/", ALLOW),
        ("rsync x/ L --newa --exclude build/", ALLOW),
        ("rsync x/ L --exclude .claude/", ALLOW),
        ("rsync x/ L --newa=v --exclude .claude/", ALLOW),
        ("rsync --newa --exclude .claude/", ALLOW),
    ]);
}

#[test]
fn rsync_value_options_from_rsyncs_table_take_their_value() {
    // rsync v3.5.2 `options.c`, `long_options[]` and `long_daemon_options[]`: each takes a
    // value, so `.claude` is that value, not a source copied to `build/.claude` (#160).
    assert_table(&[
        ("rsync --confine-root .claude x build/", ALLOW),
        ("rsync --compress-threads .claude x build/", ALLOW),
        ("rsync --zt .claude x build/", ALLOW),
        ("rsync --time-limit .claude x build/", ALLOW),
        ("rsync --nice .claude x build/", ALLOW),
        ("rsync --config .claude x build/", ALLOW),
        ("rsync --dparam .claude x build/", ALLOW),
        // Read as a flag, an unlisted option leaves `.claude` a source.
        ("rsync --unlisted-option .claude x build/", CONFIG),
        ("rsync -a x/ .claude/ --time-limit 5", CONFIG),
    ]);
}

#[test]
fn tar_extracting_into_a_config_folder_is_a_config_write() {
    assert_table(&[
        ("tar -xf x.tar -C .redkiln", CONFIG),
        ("tar -xzf x.tgz -C ./.redkiln/", CONFIG),
        ("tar xf x.tar -C .redkiln/bench", CONFIG),
        ("tar -x -f x.tar -C .claude", CONFIG),
        ("tar -xf x.tar -C.github", CONFIG),
        ("tar -xvC .cargo -f x.tar", CONFIG),
        ("tar --extract --file=x.tar --directory=.claude", CONFIG),
        ("tar --get -f x.tar --directory .anneal", CONFIG),
        ("tar --extract -f x.tar --dir=.redkiln", CONFIG),
        ("tar xfC x.tar .redkiln", CONFIG),
        ("tar -xf x.tar -C /work/proj/.redkiln", CONFIG),
        ("tar -xf x.tar -C build -C .redkiln", CONFIG),
        ("bsdtar -xf x.zip -C .claude", CONFIG),
        ("gtar -xf x.tar -C .redkiln", CONFIG),
        ("tar -xf x.tar --cd .claude", CONFIG),
        ("tar -xf x.tar --cd=.redkiln", CONFIG),
        ("cd .redkiln && tar -xf /tmp/x.tar -C bench", CONFIG),
    ]);
}

#[test]
fn tar_not_extracting_or_extracting_elsewhere_is_left_to_the_fallback() {
    assert_table(&[
        ("tar -cf /tmp/x.tar -C .redkiln .", ALLOW),
        ("tar -tf x.tar -C .redkiln", ALLOW),
        ("tar czf /tmp/x.tgz -C .claude .", ALLOW),
        ("tar -xf x.tar -C build", ALLOW),
        ("tar -xf x.tar -C .redkiln/bench/x", ALLOW),
        ("tar -xf x.tar", ALLOW),
        ("tar -xf .claude.tar -C build", ALLOW),
        ("tar -xf x.tar -C ..", ALLOW),
    ]);
}

#[test]
fn unzip_into_a_config_folder_is_a_config_write() {
    assert_table(&[
        ("unzip x.zip -d .redkiln", CONFIG),
        ("unzip -o x.zip -d ./.anneal/", CONFIG),
        ("unzip -d .redkiln/bench x.zip", CONFIG),
        ("unzip -d.cargo x.zip", CONFIG),
        ("unzip -qd .claude x.zip", CONFIG),
        ("unzip -P secret -d .github x.zip", CONFIG),
        ("unzip x.zip -d /work/proj/.claude", CONFIG),
    ]);
}

#[test]
fn unzip_elsewhere_is_left_to_the_fallback() {
    assert_table(&[
        ("unzip x.zip -d build", ALLOW),
        ("unzip x.zip", ALLOW),
        ("unzip -l .claude.zip", ALLOW),
        ("unzip -P .claude x.zip -d build", ALLOW),
        ("unzip x.zip -d .redkiln/bench/x", ALLOW),
    ]);
}
