#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(awk -F'"' '/^version =/ { print $2; exit }' Cargo.toml)"
BIN="${1:-target/x86_64-unknown-linux-musl/release/caln}"
OUT_DIR="dist"

if [ -z "$VERSION" ]; then
  echo "failed to read package version from Cargo.toml" >&2
  exit 1
fi

PACKAGE="caln_${VERSION}_amd64"

if [ ! -x "$BIN" ]; then
  echo "binary not found or not executable: $BIN" >&2
  echo "build it in GitHub Actions, then pass the binary path to this script" >&2
  exit 1
fi

mkdir -p "$ROOT/target/deb"
WORK_DIR="$(mktemp -d "$ROOT/target/deb/${PACKAGE}.XXXXXX")"
PKG_DIR="$WORK_DIR/root"
trap 'rm -rf "$WORK_DIR"' EXIT

install -Dm0755 "$BIN" "$PKG_DIR/usr/bin/caln"
install -Dm0644 packaging/systemd/caln.service "$PKG_DIR/usr/lib/systemd/user/caln.service"
install -Dm0644 README.md "$PKG_DIR/usr/share/doc/caln/README.md"
install -Dm0644 packaging/examples/env "$PKG_DIR/usr/share/doc/caln/examples/env"

INSTALLED_SIZE="$(du -ks "$PKG_DIR/usr" | awk '{print $1}')"
mkdir -p "$PKG_DIR/DEBIAN" "$OUT_DIR"
cat > "$PKG_DIR/DEBIAN/control" <<CONTROL
Package: caln
Version: $VERSION
Section: utils
Priority: optional
Architecture: amd64
Maintainer: TSK-io <noreply@github.com>
Installed-Size: $INSTALLED_SIZE
Homepage: https://github.com/TSK-io/calendar-cli
Description: Minimal YAML calendar reminder daemon
 caln reads events from a YAML file and sends email reminders through Resend.
 It installs a CLI binary and a systemd user service; user secrets stay in
 ~/.config/caln/env and are never embedded in the package.
CONTROL

DEB_PATH="$OUT_DIR/$PACKAGE.deb"
dpkg-deb --root-owner-group --build "$PKG_DIR" "$DEB_PATH"
printf '%s\n' "$DEB_PATH"
