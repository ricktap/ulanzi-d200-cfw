# Shared helpers for the d200fw scripts (sourced, not run).
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/device/target/armv7-unknown-linux-musleabihf/release/d200fw"
# Fall back to the prebuilt binary shipped in bin/ when it has not been built locally.
[ -x "$BIN" ] || BIN="$ROOT/bin/d200fw"
PORT=${D200FW_PORT:-7300}

command -v adb >/dev/null || { echo "adb not found (macOS: brew install android-platform-tools)" >&2; exit 1; }

# adb on the D200 never returns the remote exit code, so commands print what we check.
sh_dev() { adb shell "$@" | tr -d '\r'; }

need_device() {
    [ "$(adb get-state 2>/dev/null)" = "device" ] || { echo "no D200 found over adb (plug it in, check 'adb devices')" >&2; exit 1; }
    arch=$(sh_dev uname -m)
    case "$arch" in armv7*) ;; *) echo "unexpected device architecture '$arch', is this a D200?" >&2; exit 1 ;; esac
}

need_bin() {
    if [ ! -x "$BIN" ]; then
        echo "building d200fw..."
        (cd "$ROOT/device" && cargo build --release) || exit 1
    fi
}
