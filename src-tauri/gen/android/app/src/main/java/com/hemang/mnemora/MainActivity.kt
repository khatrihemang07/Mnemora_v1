package com.hemang.mnemora

import android.content.Context
import android.graphics.Color
import android.net.wifi.WifiManager
import android.os.Bundle
import android.os.PowerManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import androidx.lifecycle.Lifecycle
import com.hemang.mnemora.widget.WidgetBridge

class MainActivity : TauriActivity() {
  // Without it the Wi-Fi chip drops multicast, so mDNS (Sync finding the other device) hears nothing.
  private var multicastLock: WifiManager.MulticastLock? = null
  // What the web side last asked for (page visible with Sync on). The lock itself is held only while
  // this is true AND the activity is in front: the activity lifecycle is the authority, so the lock
  // cannot outlive the screen even if the WebView never reports the page as hidden.
  private var wantMulticast = false
  // Keeps the CPU awake for the 5 s flush after the app leaves the front (screen off would
  // otherwise let it sleep mid-session). Timed, so it can never be held longer.
  private var flushLock: PowerManager.WakeLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    // Before the Rust side starts: Sync never dials over mobile data it was not allowed to use.
    SyncBridge.watch(this)
    super.onCreate(savedInstanceState)
    // Lets the Rust side redraw the Todo widgets after the app changes Todo data.
    WidgetBridge.attach(this)
  }

  override fun onResume() {
    super.onResume()
    applyMulticastLock(true)
    // Rust starts Sync's network (or keeps it, back inside the flush window) and runs a round.
    flushLock?.takeIf { it.isHeld }?.release()
    SyncBridge.foreground(true)
  }

  override fun onPause() {
    applyMulticastLock(false)
    // Rust flushes pending changes for up to 5 s on its own timer, then stops everything.
    holdForFlush()
    SyncBridge.foreground(false)
    super.onPause()
  }

  override fun onDestroy() {
    wantMulticast = false
    applyMulticastLock(false)
    flushLock?.takeIf { it.isHeld }?.release()
    SyncBridge.unwatch(this)
    super.onDestroy()
  }

  // Only when Sync is on (the page said so), and for 6 s at most: nothing runs in the background after that.
  private fun holdForFlush() {
    if (!wantMulticast) return
    val lock = flushLock ?: (getSystemService(Context.POWER_SERVICE) as? PowerManager)
      ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mnemora:sync-flush")
      ?.also {
        it.setReferenceCounted(false)
        flushLock = it
      }
    lock?.acquire(6_000L)
  }

  // Must run on the UI thread. `inFront` is false from onPause on.
  private fun applyMulticastLock(inFront: Boolean) {
    if (wantMulticast && inFront) {
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

    // Held while the app is in front with Sync on (the web side says whether Sync is on and the page
    // visible; onResume and onPause decide the rest); released otherwise.
    @JavascriptInterface
    fun holdMulticastLock(hold: Boolean) {
      runOnUiThread {
        wantMulticast = hold
        applyMulticastLock(lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED))
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
