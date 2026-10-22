package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SampleSnapshotTest {
    @Test
    fun theSampleIsAFullTodayWithRingsTimesAndAChip() {
        val sample = SampleSnapshot.today()
        assertEquals("Today", sample.title)
        assertEquals(listOf("Overdue", "Today"), sample.groups.map { it.heading })
        val rows = sample.groups.flatMap { it.rows }
        assertEquals(setOf("p1", "p2", "p3", "none"), rows.map { it.ring }.toSet())
        assertEquals(true, rows.any { it.time != null } && rows.any { it.day != null } && rows.any { it.list != null })
        assertNull(sample.empty)
    }

    @Test
    fun theSampleNeverCarriesRealIds() {
        assertEquals(true, SampleSnapshot.today().groups.flatMap { it.rows }.all { it.id.startsWith("sample-") })
    }
}
