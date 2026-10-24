package com.hemang.mnemora

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
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
  // The page, to tell it when the local network permission dialog is answered.
  private var page: WebView? = null
  // The system's dialog is up: a missing grant is not a refusal yet.
  @Volatile private var askingLocalNetwork = false

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
    SyncBridge.foreground(true)
  }

  override fun onPause() {
    applyMulticastLock(false)
    // Rust flushes pending changes for up to 5 s on its own timer, then stops everything. Best effort:
    // no wake lock, so with the screen off the CPU may sleep before the flush finishes.
    SyncBridge.foreground(false)
    super.onPause()
  }

  override fun onDestroy() {
    wantMulticast = false
    applyMulticastLock(false)
    SyncBridge.unwatch(this)
    super.onDestroy()
  }

  private fun localNetworkGranted() =
    checkSelfPermission(LocalNetworkPolicy.PERMISSION) == PackageManager.PERMISSION_GRANTED

  private fun currentLocalNetworkState(): String {
    val asked = getSharedPreferences("sync", Context.MODE_PRIVATE).getBoolean("asked_local_network", false)
    return LocalNetworkPolicy.state(Build.VERSION.SDK_INT, localNetworkGranted(), asked && !askingLocalNetwork)
  }

  // UI thread. Shows the system's dialog when it still can; otherwise, if asked to, the app's settings
  // page (after "Don't ask again" only that can grant it). Does nothing where the permission does not
  // exist or is already granted.
  private fun askLocalNetwork(settingsIfBlocked: Boolean) {
    if (!LocalNetworkPolicy.applies(Build.VERSION.SDK_INT) || localNetworkGranted()) return
    val prefs = getSharedPreferences("sync", Context.MODE_PRIVATE)
    val asked = prefs.getBoolean("asked_local_network", false)
    if (LocalNetworkPolicy.canAsk(asked, shouldShowRequestPermissionRationale(LocalNetworkPolicy.PERMISSION))) {
      prefs.edit().putBoolean("asked_local_network", true).apply()
      askingLocalNetwork = true
      requestPermissions(arrayOf(LocalNetworkPolicy.PERMISSION), REQUEST_LOCAL_NETWORK)
    } else if (settingsIfBlocked) {
      openAppSettings()
    }
  }

  private fun openAppSettings() {
    val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", packageName, null))
    try {
      startActivity(intent)
    } catch (e: android.content.ActivityNotFoundException) {
    }
  }

  @Suppress("DEPRECATION", "OVERRIDE_DEPRECATION")
  override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    if (requestCode == REQUEST_LOCAL_NETWORK) {
      askingLocalNetwork = false
      // The page asks again what the state is (the dialog may not have paused the page).
      page?.evaluateJavascript("window.dispatchEvent(new Event('mnemora-local-network'))", null)
    }
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
    page = webView
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

    // "unneeded" before Android 17, otherwise "granted" or "denied" (Sync's local network permission).
    @JavascriptInterface
    fun localNetworkState(): String = this@MainActivity.currentLocalNetworkState()

    // Asks for the permission; with `settingsIfBlocked`, opens the app's settings page when the system
    // will not show the dialog any more. Sync calls it when it is turned on, and from its banner.
    @JavascriptInterface
    fun requestLocalNetwork(settingsIfBlocked: Boolean) {
      runOnUiThread { this@MainActivity.askLocalNetwork(settingsIfBlocked) }
    }

    // Sync was already on (app start or resume): asks once if the permission was never requested, e.g.
    // after an update to Android 17. Never asks again after an answer; the banner does that.
    @JavascriptInterface
    fun requestLocalNetworkOnce() {
      runOnUiThread {
        val asked = getSharedPreferences("sync", Context.MODE_PRIVATE).getBoolean("asked_local_network", false)
        if (LocalNetworkPolicy.shouldAskUnprompted(Build.VERSION.SDK_INT, localNetworkGranted(), asked)) {
          this@MainActivity.askLocalNetwork(false)
        }
      }
    }

    @JavascriptInterface
    fun openAppSettings() {
      runOnUiThread { this@MainActivity.openAppSettings() }
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

  private companion object {
    const val REQUEST_LOCAL_NETWORK = 4317
  }
}
