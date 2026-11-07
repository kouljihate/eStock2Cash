// The Android client talks only to this project's own /api/v1 (docs/API.md), so the app
// has no dependency to resolve and nothing to keep in step with the web build.
plugins {
    id("com.android.application") version "8.7.3" apply false
}
