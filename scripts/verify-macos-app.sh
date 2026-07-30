#!/usr/bin/env bash
# Verify that a built macOS app has a complete signature and the expected native binaries.
set -euo pipefail

APP_PATH="${1:-}"
EXPECTED_ARCH="${2:-}"
SIGNING_MODE="${3:-ad-hoc}"

fail() {
	echo "FAIL: $*" >&2
	exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || fail "macOS host required"
[[ -n "$APP_PATH" ]] || fail "usage: $0 <app-path> <arm64|x86_64> [ad-hoc|developer-id]"
[[ -d "$APP_PATH" ]] || fail "app bundle not found: $APP_PATH"
[[ "$EXPECTED_ARCH" == "arm64" || "$EXPECTED_ARCH" == "x86_64" ]] ||
	fail "expected architecture must be arm64 or x86_64"
[[ "$SIGNING_MODE" == "ad-hoc" || "$SIGNING_MODE" == "developer-id" ]] ||
	fail "signing mode must be ad-hoc or developer-id"

codesign --verify --deep --strict --verbose=4 "$APP_PATH"

SIGNING_INFO="$(codesign --display --verbose=4 "$APP_PATH" 2>&1)"
if [[ "$SIGNING_MODE" == "developer-id" ]]; then
	grep -q '^Authority=Developer ID Application:' <<<"$SIGNING_INFO" ||
		fail "app is not signed with a Developer ID Application certificate"
	grep -q '^TeamIdentifier=' <<<"$SIGNING_INFO" ||
		fail "Developer ID signature has no TeamIdentifier"
	if grep -q '^TeamIdentifier=not set$' <<<"$SIGNING_INFO"; then
		fail "Developer ID signature has no TeamIdentifier"
	fi
	spctl --assess --type execute --verbose=4 "$APP_PATH"
	xcrun stapler validate "$APP_PATH"
else
	grep -q '^Signature=adhoc$' <<<"$SIGNING_INFO" ||
		fail "app does not have the expected ad-hoc bundle signature"
fi

EXECUTABLE_NAME="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_PATH/Contents/Info.plist")"
BINARIES=(
	"$APP_PATH/Contents/MacOS/$EXECUTABLE_NAME"
	"$APP_PATH/Contents/MacOS/search"
)

for binary in "${BINARIES[@]}"; do
	[[ -x "$binary" ]] || fail "missing executable: $binary"
	ARCHES="$(lipo -archs "$binary")"
	[[ " $ARCHES " == *" $EXPECTED_ARCH "* ]] ||
		fail "$binary has architecture '$ARCHES', expected '$EXPECTED_ARCH'"
	echo "OK: $(basename "$binary") contains $EXPECTED_ARCH"
done

echo "OK: $(basename "$APP_PATH") has a valid $SIGNING_MODE signature"
