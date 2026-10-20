package com.hemang.mnemora

import android.graphics.Color
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // Exposes window.MnemoraAndroid so the web theme can restyle the system bars.
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(SystemBarsBridge(), "MnemoraAndroid")
  }

  private inner class SystemBarsBridge {
    // JavascriptInterface methods run on a WebView thread; window changes need the UI thread.
    @JavascriptInterface
    fun setSystemBars(dark: Boolean, colorHex: String) {
      runOnUiThread {
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        // "Light" appearance means dark icons, so it is the inverse of a dark theme.
        controller.isAppearanceLightStatusBars = !dark
        controller.isAppearanceLightNavigationBars = !dark
        val color = try {
          Color.parseColor(colorHex)
        } catch (e: IllegalArgumentException) {
          return@runOnUiThread
        }
        // Deprecated but still what colours the bars on API 26-34.
        @Suppress("DEPRECATION")
        window.statusBarColor = color
        @Suppress("DEPRECATION")
        window.navigationBarColor = color
      }
    }
  }
}
