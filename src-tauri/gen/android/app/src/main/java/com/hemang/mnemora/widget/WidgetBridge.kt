package com.hemang.mnemora.widget

import android.content.Context

/**
 * The Kotlin end of the JNI bridge to the app's Rust library (ADR 0014): one JSON request in, one
 * JSON reply out. Loading the library does not start Tauri, so a receiver can call it with the
 * app closed.
 */
object WidgetBridge {
    private val loaded: Boolean by lazy {
        try {
            System.loadLibrary("mnemora_lib")
            true
        } catch (e: UnsatisfiedLinkError) {
            false
        }
    }

    @Volatile
    private var appContext: Context? = null

    /** Lets Rust call [onTodoChanged]. Called by the app's activity. */
    @JvmStatic external fun nativeAttach()

    @JvmStatic external fun nativeCall(dataDir: String, request: String): String?

    /** Remembers the application context and gives Rust a way back. Safe to call repeatedly. */
    fun attach(context: Context) {
        appContext = context.applicationContext
        if (loaded) {
            try {
                nativeAttach()
            } catch (e: UnsatisfiedLinkError) {
                // A build without the bridge: the widget shows its error state instead.
            }
        }
    }

    /** Sends [request]; null when the library is missing or the call failed. Never throws. */
    fun call(context: Context, request: String): String? {
        if (!loaded) return null
        return try {
            nativeCall(context.dataDir.absolutePath, request)
        } catch (e: Throwable) {
            null
        }
    }

    /** Called by Rust, on any thread, after the app changes Todo data. */
    @JvmStatic
    fun onTodoChanged() {
        appContext?.let { WidgetUpdater.signalChanged(it) }
    }
}
