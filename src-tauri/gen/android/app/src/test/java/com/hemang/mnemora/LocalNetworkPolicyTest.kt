package com.hemang.mnemora

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalNetworkPolicyTest {
    @Test
    fun theLocalNetworkPermissionExistsFromAndroid17() {
        assertFalse(LocalNetworkPolicy.applies(36))
        assertTrue(LocalNetworkPolicy.applies(37))
        assertTrue(LocalNetworkPolicy.applies(40))
    }

    @Test
    fun beforeAndroid17NothingIsNeededWhateverTheGrantSays() {
        assertEquals(LocalNetworkPolicy.UNNEEDED, LocalNetworkPolicy.state(35, granted = false))
        assertEquals(LocalNetworkPolicy.UNNEEDED, LocalNetworkPolicy.state(36, granted = true))
    }

    @Test
    fun onAndroid17TheStateIsTheGrant() {
        assertEquals(LocalNetworkPolicy.GRANTED, LocalNetworkPolicy.state(37, granted = true))
        assertEquals(LocalNetworkPolicy.DENIED, LocalNetworkPolicy.state(37, granted = false))
    }

    @Test
    fun theDialogCanBeShownTheFirstTimeAndWhileARationaleIsDue() {
        assertTrue(LocalNetworkPolicy.canAsk(askedBefore = false, rationaleDue = false))
        assertTrue(LocalNetworkPolicy.canAsk(askedBefore = true, rationaleDue = true))
    }

    @Test
    fun afterDontAskAgainOnlyTheSettingsPageCanGrantIt() {
        assertFalse(LocalNetworkPolicy.canAsk(askedBefore = true, rationaleDue = false))
    }
}
