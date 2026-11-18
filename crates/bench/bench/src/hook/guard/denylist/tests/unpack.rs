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
        ("cd .redkiln && rsync -a /tmp/x/ bench/", CONFIG),
        ("bash -c 'rsync -a x/ .redkiln/'", CONFIG),
    ]);
}

#[test]
fn rsync_moving_config_away_is_a_config_write() {
    assert_table(&[
        ("rsync --remove-source-files .claude/settings.json /tmp/x", CONFIG),
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
