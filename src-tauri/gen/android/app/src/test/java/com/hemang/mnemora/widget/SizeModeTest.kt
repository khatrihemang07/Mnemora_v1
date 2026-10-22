package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Test

class SizeModeTest {
    @Test
    fun thresholdsArePinned() {
        assertEquals(180f, SizeMode.LIST_MIN_WIDTH_DP, 0f)
        assertEquals(110f, SizeMode.LIST_MIN_HEIGHT_DP, 0f)
        assertEquals(100f, SizeMode.PILL_MIN_WIDTH_DP, 0f)
    }

    @Test
    fun theListNeedsAtLeastThreeByTwo() {
        assertEquals(SizeMode.LIST, SizeMode.of(180f, 110f))
        assertEquals(SizeMode.LIST, SizeMode.of(320f, 400f))
        // Wide but short (4x1) and tall but narrow (2x4) are not lists.
        assertEquals(SizeMode.PILL, SizeMode.of(320f, 109f))
        assertEquals(SizeMode.PILL, SizeMode.of(179f, 300f))
    }

    @Test
    fun thePillNeedsAHundredDpOfWidth() {
        assertEquals(SizeMode.PILL, SizeMode.of(100f, 40f))
        assertEquals(SizeMode.PILL, SizeMode.of(150f, 150f))
        assertEquals(SizeMode.PLUS, SizeMode.of(99.9f, 300f))
        assertEquals(SizeMode.PLUS, SizeMode.of(40f, 40f))
    }

    @Test
    fun theTypicalLauncherCells() {
        // 1x1 .. 4x3 on a phone whose cells are about 73x118 dp.
        assertEquals(SizeMode.PLUS, SizeMode.of(57f, 102f))
        assertEquals(SizeMode.PILL, SizeMode.of(130f, 102f))
        assertEquals(SizeMode.PILL, SizeMode.of(130f, 220f))
        assertEquals(SizeMode.PILL, SizeMode.of(276f, 102f))
        assertEquals(SizeMode.LIST, SizeMode.of(203f, 220f))
        assertEquals(SizeMode.LIST, SizeMode.of(276f, 340f))
    }

    @Test
    fun eachLayoutBreakpointIsTheModeItStandsFor() {
        assertEquals(listOf(SizeMode.PLUS, SizeMode.PILL, SizeMode.LIST), SizeMode.BREAKPOINTS.map { it.mode })
        for (b in SizeMode.BREAKPOINTS) assertEquals(b.mode, SizeMode.of(b.widthDp, b.heightDp))
    }
}
