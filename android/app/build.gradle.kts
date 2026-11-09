plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "org.wanten.remoteinput"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.wanten.remoteinput"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.3.0"
    }

    signingConfigs {
        create("release") {
            storeFile = file("release.jks")
            storePassword = "TDoeuxtIUWFKIohab0aEAoZ"
            keyAlias = "remote-input"
            keyPassword = "TDoeuxtIUWFKIohab0aEAoZ"
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("release")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.camera:camera-core:1.6.2")
    implementation("androidx.camera:camera-camera2:1.6.2")
    implementation("androidx.camera:camera-lifecycle:1.6.2")
    implementation("androidx.camera:camera-view:1.6.2")
    implementation("com.google.zxing:core:3.5.4")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
