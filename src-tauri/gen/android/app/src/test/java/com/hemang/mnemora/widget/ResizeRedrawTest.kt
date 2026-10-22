package com.hemang.mnemora.widget

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ResizeRedrawTest {
    @Test
    fun aProcessThatNeverDrewDraws() {
        assertTrue(ResizeRedraw.due(lastDrawAt = 0L, now = 5_000_000L))
    }

    @Test
    fun theTicksOfADragRightAfterADrawDoNot() {
        assertFalse(ResizeRedraw.due(lastDrawAt = 5_000_000L, now = 5_000_400L))
        assertFalse(ResizeRedraw.due(lastDrawAt = 5_000_000L, now = 5_000_000L + ResizeRedraw.FRESH_MS))
    }

    @Test
    fun anOldDrawOrAClockThatWentBackDraws() {
        assertTrue(ResizeRedraw.due(lastDrawAt = 5_000_000L, now = 5_000_001L + ResizeRedraw.FRESH_MS))
        assertTrue(ResizeRedraw.due(lastDrawAt = 5_000_000L, now = 4_000_000L))
    }
}
