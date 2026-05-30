#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NAME="$(sed -n 's/^name = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
TARGET="${TARGET:-}"
OUT_DIR="${OUT_DIR:-dist}"
MAINTAINER="${DEB_MAINTAINER:-TSK-io <noreply@github.com>}"

if [ -z "$NAME" ] || [ -z "$VERSION" ]; then
  echo "failed to read package name/version from Cargo.toml" >&2
  exit 1
fi

target_triple() {
  if [ -n "$TARGET" ]; then
    printf '%s\n' "$TARGET"
  else
    rustc -vV | sed -n 's/^host: //p'
  fi
}

deb_arch_for_target() {
  case "$1" in
    x86_64-*) printf 'amd64\n' ;;
    aarch64-*) printf 'arm64\n' ;;
    armv7-*) printf 'armhf\n' ;;
    *)
      echo "unsupported target '$1'; set DEB_ARCH explicitly" >&2
      exit 1
      ;;
  esac
}

TRIPLE="$(target_triple)"
DEB_ARCH="${DEB_ARCH:-$(deb_arch_for_target "$TRIPLE")}"
PACKAGE="${NAME}_${VERSION}_${DEB_ARCH}"

if [ -n "${BIN_PATH:-}" ]; then
  BIN="$BIN_PATH"
elif [ -n "$TARGET" ]; then
  BIN="target/$TARGET/release/$NAME"
else
  BIN="target/release/$NAME"
fi

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  if [ -n "$TARGET" ]; then
    cargo build --release --target "$TARGET"
  else
    cargo build --release
  fi
fi

if [ ! -x "$BIN" ]; then
  echo "binary not found or not executable: $BIN" >&2
  exit 1
fi

mkdir -p "$ROOT/target/deb"
WORK_DIR="$(mktemp -d "$ROOT/target/deb/${PACKAGE}.XXXXXX")"
PKG_DIR="$WORK_DIR/root"
trap 'rm -rf "$WORK_DIR"' EXIT

install -Dm0755 "$BIN" "$PKG_DIR/usr/bin/$NAME"
install -Dm0644 packaging/systemd/caln.service "$PKG_DIR/usr/lib/systemd/user/caln.service"
install -Dm0644 README.md "$PKG_DIR/usr/share/doc/$NAME/README.md"
install -Dm0644 packaging/examples/env "$PKG_DIR/usr/share/doc/$NAME/examples/env"

INSTALLED_SIZE="$(du -ks "$PKG_DIR/usr" | awk '{print $1}')"
mkdir -p "$PKG_DIR/DEBIAN" "$OUT_DIR"
cat > "$PKG_DIR/DEBIAN/control" <<CONTROL
Package: $NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Maintainer: $MAINTAINER
Installed-Size: $INSTALLED_SIZE
Homepage: https://github.com/TSK-io/calendar-cli
Description: Minimal YAML calendar reminder daemon
 caln reads events from a YAML file and sends email reminders through Resend.
 It installs a CLI binary and a systemd user service; user secrets stay in
 ~/.config/caln/env and are never embedded in the package.
CONTROL

DEB_PATH="$OUT_DIR/${PACKAGE}.deb"
dpkg-deb --root-owner-group --build "$PKG_DIR" "$DEB_PATH"
printf '%s\n' "$DEB_PATH"
