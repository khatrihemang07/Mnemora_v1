package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class UndoBarTest {
    @Test
    fun theBarLastsAboutFourSecondsFromTheTick() {
        assertEquals(1_004_000L, UndoBar.expiresAt(1_000_000L))
    }

    @Test
    fun theBarIsHiddenByTheClockSoADeadProcessCannotLeaveItUp() {
        val until = UndoBar.expiresAt(1_000_000L)
        assertTrue(UndoBar.isShown(until, 1_000_001L))
        assertTrue(UndoBar.isShown(until, 1_003_999L))
        assertFalse(UndoBar.isShown(until, 1_004_000L))
        assertFalse(UndoBar.isShown(until, 9_000_000L))
    }

    @Test
    fun aClockSetBackDoesNotKeepTheBarForever() {
        // The bar was due to end at 1_004_000; the clock jumped back a day.
        assertFalse(UndoBar.isShown(until = 1_004_000L, now = 1_004_000L - 86_400_000L))
    }
}
