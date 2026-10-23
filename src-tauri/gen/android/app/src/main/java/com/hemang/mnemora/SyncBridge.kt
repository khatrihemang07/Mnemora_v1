package com.hemang.mnemora

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest

/**
 * The Kotlin end of Sync's lifecycle (#74): tells the Rust library when the activity is in front
 * and what kind of network the phone is on. Rust starts and stops Sync's network from these (and
 * runs the 5 s background flush on its own timer), so the page being suspended changes nothing.
 * Every call is safe without the library (a build without the bridge): it just does nothing.
 */
object SyncBridge {
    private val loaded: Boolean by lazy {
        try {
            System.loadLibrary("mnemora_lib")
            true
        } catch (e: UnsatisfiedLinkError) {
            false
        }
    }

    @JvmStatic external fun nativeForeground(front: Boolean)

    @JvmStatic external fun nativeLink(code: Int)

    private var manager: ConnectivityManager? = null
    private var callback: ConnectivityManager.NetworkCallback? = null
    // The non-VPN networks that are up, by handle. Touched from the connectivity thread and from
    // watch/unwatch, so guarded by itself.
    private val nets = HashMap<Network, SyncLink.Net>()

    fun foreground(front: Boolean) {
        if (!loaded) return
        try {
            nativeForeground(front)
        } catch (e: UnsatisfiedLinkError) {
        }
    }

    private fun report() {
        if (!loaded) return
        val code = synchronized(nets) { SyncLink.of(nets.values) }
        try {
            nativeLink(code)
        } catch (e: UnsatisfiedLinkError) {
        }
    }

    private fun describe(caps: NetworkCapabilities) = SyncLink.Net(
        wifi = caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI),
        ethernet = caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET),
        cellular = caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR),
    )

    /**
     * Starts watching the phone's networks and reports the current state at once. VPNs (Tailscale)
     * are left out of the request, so what is reported is the network underneath. Safe to call
     * again; [unwatch] stops it.
     */
    fun watch(context: Context) {
        if (callback != null) return
        val cm = context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
          ?: return
        val request = NetworkRequest.Builder()
          .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
          .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
          .build()
        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                // Fires for every change in signal strength too: only a different kind of network counts.
                val now = describe(caps)
                val before = synchronized(nets) { nets.put(network, now) }
                if (before != now) report()
            }

            override fun onLost(network: Network) {
                synchronized(nets) { nets.remove(network) }
                report()
            }
        }
        try {
            cm.registerNetworkCallback(request, cb)
        } catch (e: SecurityException) {
            // No network-state permission: Sync is never blocked on a guess.
            return
        }
        manager = cm
        callback = cb
        // Until the first callback arrives the state is unknown to Rust, which never blocks Sync.
    }

    fun unwatch() {
        val cb = callback ?: return
        try {
            manager?.unregisterNetworkCallback(cb)
        } catch (e: IllegalArgumentException) {
        }
        callback = null
        manager = null
        synchronized(nets) { nets.clear() }
    }
}
