ls "$HOME/mnt"
V=dev; S=none;
B="$HOME/.weighin/$V/weighin"
if [ ! -x "$B" ] && [ "$S" != none ]; then
  mkdir -p "${B%/*}" &&
    curl -fsSL --max-time 90 -o "$B.part" "https://github.com/Wet-Ink-Corporation/weigh-in/releases/download/v$V/weighin-x86_64-unknown-linux-musl" &&
    echo "$S  $B.part" | sha256sum -c --quiet - &&
    chmod +x "$B.part" && mv "$B.part" "$B" ||
    echo "download failed or did not match the pinned SHA-256"
fi
if [ ! -x "$B" ]; then
  B=
  for c in "$HOME"/mnt/*/plugin/libexec/x86_64-unknown-linux-musl/weighin "$HOME"/mnt/*/libexec/x86_64-unknown-linux-musl/weighin; do
    [ -f "$c" ] || continue
    v=$("$c" --version 2>/dev/null)
    if [ "$V" = dev ] || [ "$v" = "weighin $V" ]; then B=$c; break; fi
    echo "skipped $c (${v:-does not run}; this release is $V)"
  done
fi
if [ -n "$B" ]; then echo "B=$B"; "$B" --version; else echo "B= (no Weigh-In binary)"; fi
