#!/bin/sh
set -eu

release_base=${ANKI_AUTO_RELEASE_BASE:-https://github.com/moshiroi/anki-auto/releases}
install_dir=${ANKI_AUTO_INSTALL_DIR:-$HOME/.local/share/anki-auto}
version=${ANKI_AUTO_VERSION:-}
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT HUP INT TERM

if [ -z "$version" ]; then
    latest=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$release_base/latest")
    version=${latest##*/}
fi
case "$version" in v[0-9]*) ;; *) echo 'Cannot find a published anki-auto release.' >&2; exit 1 ;; esac
case "$version" in *[!a-zA-Z0-9._-]*) echo 'Invalid release version.' >&2; exit 1 ;; esac
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) target=aarch64-apple-darwin ;;
    Darwin:x86_64) target=x86_64-apple-darwin ;;
    Linux:x86_64) target=x86_64-unknown-linux-musl ;;
    Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-musl ;;
    *) echo 'Unsupported platform. See the source-install instructions in the README.' >&2; exit 1 ;;
esac
archive=anki-auto-$target.tar.gz
url=$release_base/download/$version
curl -fsSL "$url/$archive" -o "$temp_dir/$archive"
curl -fsSL "$url/$archive.sha256" -o "$temp_dir/checksum"
expected=$(awk 'NR == 1 { print $1 }' "$temp_dir/checksum")
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$temp_dir/$archive" | awk '{ print $1 }')
else
    actual=$(shasum -a 256 "$temp_dir/$archive" | awk '{ print $1 }')
fi
if [ "$expected" != "$actual" ]; then
    echo 'Checksum mismatch; nothing was installed.' >&2
    exit 1
fi
mkdir "$temp_dir/unpacked"
tar -xzf "$temp_dir/$archive" -C "$temp_dir/unpacked"
# Check compatibility before replacing an existing installation.
chmod +x "$temp_dir/unpacked/anki-auto"
"$temp_dir/unpacked/anki-auto" --version
mkdir -p "$install_dir"
cp "$temp_dir/unpacked/anki-auto" "$install_dir/anki-auto.new"
chmod +x "$install_dir/anki-auto.new"
mv -f "$install_dir/anki-auto.new" "$install_dir/anki-auto"
cp "$temp_dir/unpacked/basic.json" "$install_dir/basic.json"
cp "$temp_dir/unpacked/LICENSE" "$install_dir/LICENSE"

if [ "${ANKI_AUTO_NO_MODIFY_PATH:-0}" != 1 ]; then
    case "${SHELL:-}" in
        */zsh) profile=$HOME/.zshrc ;;
        */bash)
            if [ "$(uname -s)" = Darwin ]; then profile=$HOME/.bash_profile; else profile=$HOME/.bashrc; fi ;;
        *) profile='' ;;
    esac
    if [ -n "$profile" ]; then
        # Single-quote the path so shell metacharacters in it remain literal.
        quoted=$(printf '%s' "$install_dir" | sed "s/'/'\\\\''/g")
        path_line="export PATH='$quoted':\"\$PATH\""
        if ! grep -Fqx "$path_line" "$profile" 2>/dev/null; then
            printf '\n# anki-auto\n%s\n' "$path_line" >> "$profile"
        fi
        printf 'Added the install directory to %s. Open a new terminal to use anki-auto.\n' "$profile"
    else
        printf 'Add this directory to your shell PATH: %s\n' "$install_dir"
    fi
fi
printf 'Installed %s in %s\nExample JSON: %s/basic.json\n' "$version" "$install_dir" "$install_dir"
printf 'Next: keep Anki open and run anki-auto ping.\n'
