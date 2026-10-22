package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.ZoneId
import java.time.ZonedDateTime

class MidnightTest {
    private fun ms(text: String, zone: String) = ZonedDateTime.parse(text).withZoneSameInstant(ZoneId.of(zone)).toInstant().toEpochMilli()

    @Test
    fun isFiveSecondsPastTheNextLocalMidnight() {
        val zone = ZoneId.of("Asia/Kolkata")
        val now = ms("2026-10-21T14:05:00+05:30", "Asia/Kolkata")
        val expected = ms("2026-10-22T00:00:05+05:30", "Asia/Kolkata")
        assertEquals(expected, Midnight.nextAfter(now, zone))
    }

    @Test
    fun justAfterMidnightMeansTomorrowsMidnight() {
        val zone = ZoneId.of("Europe/London")
        val now = ms("2026-10-21T00:00:01+01:00", "Europe/London")
        assertEquals(ms("2026-10-22T00:00:05+01:00", "Europe/London"), Midnight.nextAfter(now, zone))
    }

    @Test
    fun followsTheClockAcrossDaylightSaving() {
        val zone = ZoneId.of("America/New_York")
        // Spring forward on 8 March 2026: that day is 23 hours long.
        val evening = ms("2026-03-07T22:00:00-05:00", "America/New_York")
        assertEquals(ms("2026-03-08T00:00:05-05:00", "America/New_York"), Midnight.nextAfter(evening, zone))
        val noon = ms("2026-03-08T12:00:00-04:00", "America/New_York")
        assertEquals(ms("2026-03-09T00:00:05-04:00", "America/New_York"), Midnight.nextAfter(noon, zone))
    }

    @Test
    fun theJobWaitsUntilMidnightPlusSlack() {
        val zone = ZoneId.of("Asia/Kolkata")
        val now = ms("2026-10-22T23:58:00+05:30", "Asia/Kolkata")
        // Two minutes to midnight and five seconds of slack.
        assertEquals(125_000L, Midnight.delayFrom(now, zone))
    }

    @Test
    fun theDelayAcrossASpringForwardDayIsTwentyThreeHoursLong() {
        val zone = ZoneId.of("America/New_York")
        val midnight = ms("2026-03-08T00:00:00-05:00", "America/New_York")
        assertEquals(23 * 3_600_000L + 5_000L, Midnight.delayFrom(midnight, zone))
    }
}
