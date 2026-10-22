package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HeaderDateTest {
    @Test
    fun onlyTodayShowsTheDateAndOnlyWhenWideEnough() {
        assertTrue(HeaderDate.shows("today", true))
        assertFalse(HeaderDate.shows("today", false))
        for (v in listOf("upcoming", "inbox", "all", "list:abc")) assertFalse(HeaderDate.shows(v, true))
    }

    @Test
    fun theFormatIsTheLocalesShortDateAfterAQuotedDot() {
        assertEquals("EEEd", HeaderDate.SKELETON)
        assertEquals("'·' EEE d", HeaderDate.pattern("EEE d"))
    }

    @Test
    fun theDateGivesWayBeforeTheNameAndOnlyWhenShown() {
        assertFalse(HeaderDate.nameIsFlexible(true))
        assertTrue(HeaderDate.nameIsFlexible(false))
    }
}
