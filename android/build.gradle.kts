plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "mba.robin.ondroidmediaforge"
    compileSdk = 34

    defaultConfig {
        minSdk = 31
        targetSdk = 34

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
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
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")

    // LiteRT scaffold — AD-3 permits a published binary as a scaffold;
    // T24 substitutes the from-source build.
    implementation("com.google.ai.edge.litert:litert:0.9.0")

    // Play Billing for entitlement
    implementation("com.android.billingclient:billing-ktx:6.1.0")

    // RevenueCat for entitlement management
    implementation("com.revenuecat.purchases:purchases:6.9.0")

    // WorkManager for foreground job service
    implementation("androidx.work:work-runtime-ktx:2.9.0")
}
