package com.hemang.mnemora

import android.graphics.Color
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import com.hemang.mnemora.widget.WidgetBridge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Lets the Rust side redraw the Todo widgets after the app changes Todo data.
    WidgetBridge.attach(this)
  }

  // Exposes window.MnemoraAndroid so the web side can restyle the system bars and leave the app.
  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(MnemoraBridge(), "MnemoraAndroid")
  }

  private inner class MnemoraBridge {
    // Back at the root leaves like any Android app, keeping the process alive (see #51).
    @JavascriptInterface
    fun moveTaskToBack() {
      runOnUiThread { this@MainActivity.moveTaskToBack(true) }
    }

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
