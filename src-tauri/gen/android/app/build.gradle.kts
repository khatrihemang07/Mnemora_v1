import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Sandbox is a separate app (own id, label, data dir) so it never installs over Production.
val isSandbox = System.getenv("MNEMORA_ANDROID_SANDBOX") == "1" ||
    project.findProperty("mnemora.sandbox") == "true"
val isReleaseBuild = gradle.startParameter.taskNames.any { it.contains("release", ignoreCase = true) }

android {
    compileSdk = 36
    namespace = "com.hemang.mnemora"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        // The label is a placeholder because strings.xml's app_name is shared by both apps.
        manifestPlaceholders["appLabel"] = if (isSandbox) "Mnemora Sandbox" else "@string/app_name"
        applicationId = "com.hemang.mnemora"
        // The Todo widget's picker label and its SANDBOX tag (Sandbox is a separate app).
        resValue("string", "todo_widget_label", if (isSandbox) "Todo (Sandbox)" else "Todo")
        // The picker's static preview cannot branch at run time, so it takes the Sandbox tag from here.
        resValue("string", "todo_widget_tag_text", if (isSandbox) "SANDBOX" else "")
        resValue("color", "todo_widget_tag_fill", if (isSandbox) "#FFE48E26" else "#00000000")
        buildConfigField("boolean", "SANDBOX", isSandbox.toString())
        if (isSandbox) applicationIdSuffix = ".sandbox"
        minSdk = 26
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        // arm64 only: keeps builds fast and the NDK pruned (scripts/setup-android.sh).
        ndk { abiFilters += listOf("arm64-v8a") }
    }
    // Release signing: keystore path/alias from keystore.properties (no secrets), password from
    // MNEMORA_KEYSTORE_PASSWORD, which `make android` exports from the macOS Keychain.
    val keystoreProps = Properties().apply {
        val f = rootProject.file("keystore.properties")
        if (f.exists()) f.inputStream().use { load(it) }
    }
    val keystorePassword = System.getenv("MNEMORA_KEYSTORE_PASSWORD")
    if (isReleaseBuild && (keystorePassword == null || keystoreProps.getProperty("storeFile") == null)) {
        throw GradleException(
            "Release signing is not set up (MNEMORA_KEYSTORE_PASSWORD or keystore.properties missing). " +
                "Run `make setup` (scripts/setup-signing.sh), and build with `make android`."
        )
    }
    signingConfigs {
        if (keystorePassword != null && keystoreProps.getProperty("storeFile") != null) {
            create("release") {
                storeFile = file(keystoreProps.getProperty("storeFile"))
                storePassword = keystorePassword
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystorePassword
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
            }
        }
        getByName("release") {
            signingConfig = signingConfigs.findByName("release")
            // R8 off: it's the heaviest Gradle step (RAM/CPU) and app code is almost all Rust.
            isMinifyEnabled = false
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    // The Android SDK's org.json is a stub in local unit tests; the widget's parsing needs the real one.
    testImplementation("org.json:json:20240303")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")