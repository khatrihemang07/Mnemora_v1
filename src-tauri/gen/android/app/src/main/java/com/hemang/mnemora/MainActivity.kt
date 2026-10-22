package com.hemang.mnemora

import android.content.Context
import android.graphics.Color
import android.net.wifi.WifiManager
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import com.hemang.mnemora.widget.WidgetBridge

class MainActivity : TauriActivity() {
  // Without it the Wi-Fi chip drops multicast, so mDNS (Sync finding the other device) hears nothing.
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Lets the Rust side redraw the Todo widgets after the app changes Todo data.
    WidgetBridge.attach(this)
  }

  override fun onDestroy() {
    multicastLock?.takeIf { it.isHeld }?.release()
    super.onDestroy()
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

    // Held while the app is in front with Sync on (the web side decides); released otherwise.
    @JavascriptInterface
    fun holdMulticastLock(hold: Boolean) {
      runOnUiThread {
        if (hold) {
          val lock = multicastLock ?: (applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
            ?.createMulticastLock("mnemora-sync")
            ?.also {
              it.setReferenceCounted(false)
              multicastLock = it
            }
          if (lock != null && !lock.isHeld) lock.acquire()
        } else {
          multicastLock?.takeIf { it.isHeld }?.release()
        }
      }
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
