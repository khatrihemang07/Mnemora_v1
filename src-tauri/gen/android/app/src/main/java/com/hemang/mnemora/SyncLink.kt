package com.hemang.mnemora

/**
 * What kind of network the phone is on, for Sync's "Allow over mobile data". The codes are the ones
 * the Rust side reads (`Link::from_code`). Tailscale is a VPN on top of a real network, so VPN
 * networks are never looked at: the networks underneath decide.
 */
object SyncLink {
    const val OFFLINE = 0
    /** Wi-Fi, Ethernet or anything else that is not mobile data. */
    const val WIFI = 1
    const val MOBILE = 2

    /** One non-VPN network that has internet. */
    data class Net(val wifi: Boolean, val ethernet: Boolean, val cellular: Boolean)

    /** Wi-Fi or Ethernet anywhere wins over mobile data; mobile data counts only when it is all there is. */
    fun of(nets: Collection<Net>): Int = when {
        nets.isEmpty() -> OFFLINE
        nets.any { it.wifi || it.ethernet || !it.cellular } -> WIFI
        else -> MOBILE
    }
}
