#!/usr/bin/env bash
# Builds the Android client and drops the APK in android/dist/.
#
# The machine's own `java` is a JRE 1.8 without javac, so the JDK that ships with
# Android Studio is used for the build regardless of what JAVA_HOME says. Gradle comes
# from the wrapper cache when there is no `gradle` on PATH.
set -euo pipefail
cd "$(dirname "$0")/.."

export JAVA_HOME="${S2C_JAVA_HOME:-C:/Program Files/Android/Android Studio/jbr}"

if command -v gradle >/dev/null 2>&1; then
  GRADLE=gradle
else
  GRADLE=$(ls -d "$HOME"/.gradle/wrapper/dists/gradle-*/**/gradle-*/bin/gradle 2>/dev/null | sort -V | tail -1)
fi
if [ -z "${GRADLE:-}" ]; then
  echo "No gradle found: install it, or open android/ in Android Studio once." >&2
  exit 1
fi

VERSION=$(grep -m1 'versionName' android/app/build.gradle.kts | sed 's/.*"\(.*\)".*/\1/')

echo "gradle:  $GRADLE"
echo "javac:   $JAVA_HOME/bin/javac"
echo "version: $VERSION"

"$GRADLE" -p android --console=plain "${1:-assembleDebug}"

mkdir -p android/dist
cp android/app/build/outputs/apk/debug/app-debug.apk "android/dist/s2c-maroc-${VERSION}-debug.apk"
ls -la "android/dist/s2c-maroc-${VERSION}-debug.apk"
