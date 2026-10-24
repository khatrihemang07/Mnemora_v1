package com.hemang.mnemora

import com.hemang.mnemora.SyncLink.Net
import org.junit.Assert.assertEquals
import org.junit.Test

class SyncLinkTest {
    private val wifi = Net(wifi = true, ethernet = false, cellular = false)
    private val ethernet = Net(wifi = false, ethernet = true, cellular = false)
    private val cellular = Net(wifi = false, ethernet = false, cellular = true)
    private val other = Net(wifi = false, ethernet = false, cellular = false)

    @Test
    fun noNetworkIsOffline() {
        assertEquals(SyncLink.OFFLINE, SyncLink.of(emptyList()))
    }

    @Test
    fun wifiAndEthernetAreNotMobileData() {
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(wifi)))
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(ethernet)))
    }

    @Test
    fun cellularAloneIsMobileData() {
        assertEquals(SyncLink.MOBILE, SyncLink.of(listOf(cellular)))
    }

    @Test
    fun wifiBesideCellularWins() {
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(cellular, wifi)))
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(wifi, cellular)))
    }

    @Test
    fun otherTransportsLikeBluetoothTetherAreNotMobileData() {
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(other)))
        assertEquals(SyncLink.WIFI, SyncLink.of(listOf(cellular, other)))
    }
}
