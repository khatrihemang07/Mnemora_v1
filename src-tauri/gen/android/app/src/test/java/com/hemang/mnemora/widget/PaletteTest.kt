package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Test

class PaletteTest {
    private val palette = Palette(
        background = 0xFFFFFFFF.toInt(), surface2 = 0xFFF5EFE9.toInt(), ink = 0xFF1D1712.toInt(),
        muted = 0xFF6A615A.toInt(), faint = 0xFF8C857E.toInt(), danger = 0xFFC2181D.toInt(),
        warning = 0xFFE48E26.toInt(), accent = 0xFF615CDC.toInt(), todo = 0xFF00A08F.toInt(),
    )

    @Test
    fun priorityRingsKeepTheirMeaning() {
        assertEquals(palette.danger, palette.ringEdge("p1", false))
        assertEquals(palette.warning, palette.ringEdge("p2", false))
        assertEquals(palette.accent, palette.ringEdge("p3", false))
        assertEquals(palette.faint, palette.ringEdge("none", false))
    }

    @Test
    fun anOverdueTaskWithoutAPriorityGetsAReddishEdge() {
        val edge = palette.ringEdge("none", true)
        assertEquals(Palette.blend(palette.danger, palette.faint, 0.7f), edge)
        assert(edge != palette.faint && edge != palette.danger)
    }

    @Test
    fun anOverdueTaskKeepsItsPriorityColour() {
        assertEquals(palette.danger, palette.ringEdge("p1", true))
        assertEquals(palette.warning, palette.ringEdge("p2", true))
    }

    @Test
    fun onlyPriorityRingsAreFilled() {
        assertEquals(0, palette.ringFill("none"))
        assertEquals(0x1E, palette.ringFill("p1") ushr 24)
        assertEquals(palette.danger and 0xFFFFFF, palette.ringFill("p1") and 0xFFFFFF)
    }

    @Test
    fun blendWeightsTheFirstColour() {
        assertEquals(0xFF000000.toInt(), Palette.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 1f))
        assertEquals(0xFFFFFFFF.toInt(), Palette.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0f))
        assertEquals(0xFF7F7F7F.toInt(), Palette.blend(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0.5f))
    }
}
