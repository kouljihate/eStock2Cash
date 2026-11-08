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
  # Android Gradle Plugin 8.7 runs on Gradle 8.x only, so a 9.x wrapper cached here by
  # another project must not be picked: prefer the newest 8.x, then anything else.
  GRADLE=$(ls -d "$HOME"/.gradle/wrapper/dists/gradle-8.*/**/gradle-8.*/bin/gradle 2>/dev/null | sort -V | tail -1)
  [ -n "${GRADLE:-}" ] || GRADLE=$(ls -d "$HOME"/.gradle/wrapper/dists/gradle-*/**/gradle-*/bin/gradle 2>/dev/null | sort -V | tail -1)
fi
if [ -z "${GRADLE:-}" ]; then
  echo "No gradle found: install it, or open android/ in Android Studio once." >&2
  exit 1
fi

VERSION=$(grep -m1 'versionName' android/app/build.gradle.kts | sed 's/.*"\(.*\)".*/\1/')

echo "gradle:  $GRADLE"
echo "javac:   $JAVA_HOME/bin/javac"
echo "version: $VERSION"

TASK="${1:-assembleDebug}"
case "$TASK" in
  assembleRelease) VARIANT=release ;;
  *) VARIANT=debug ;;
esac

"$GRADLE" -p android --console=plain "$TASK"

mkdir -p android/dist
cp "android/app/build/outputs/apk/$VARIANT/app-$VARIANT.apk" "android/dist/s2c-maroc-${VERSION}-${VARIANT}.apk"
ls -la "android/dist/s2c-maroc-${VERSION}-${VARIANT}.apk"
