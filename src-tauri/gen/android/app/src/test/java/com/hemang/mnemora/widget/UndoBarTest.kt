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

    @Test
    fun theFourSecondsStartWhenTheBarIsDrawnNotWhenTheTickWasAsked() {
        val asked = UndoState("Completed", null, "tok", UndoBar.expiresAt(1_000_000L))
        // The read and the draw took 1.4 s.
        val shown = asked.startedAt(1_001_400L)
        assertEquals(1_005_400L, shown.until)
        assertTrue(UndoBar.isShown(shown.until, 1_005_399L))
        assertFalse(UndoBar.isShown(shown.until, 1_005_400L))
        assertEquals("tok", shown.token)
    }

    @Test
    fun anAnswerFromTheDialogGetsExtraTimeForTheLauncherToCatchUp() {
        val shown = UndoState("Completed", null, "tok", 0L).startedAt(1_000_000L, UndoBar.DIALOG_GRACE_MS)
        assertEquals(1_005_500L, shown.until)
        assertTrue(UndoBar.isShown(shown.until, 1_000_000L))
        assertFalse(UndoBar.isShown(shown.until, 1_005_500L))
    }
}
