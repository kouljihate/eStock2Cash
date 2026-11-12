plugins {
    id("com.android.application")
}

android {
    namespace = "ma.s2c.maroc"
    compileSdk = 35

    defaultConfig {
        applicationId = "ma.s2c.maroc"
        minSdk = 24
        targetSdk = 35
        versionCode = 6
        versionName = "0.4.0"
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
        }
        release {
            // Signed with the debug key for now: this build is handed over by cable or
            // file, not through a store, and a release key belongs to the owner.
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        // Nothing here ships to a store yet; keep the build green on the essentials.
        abortOnError = false
    }
}
