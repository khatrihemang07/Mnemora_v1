package com.hemang.mnemora.widget

import java.time.Instant
import java.time.ZoneId

/** When the Todo widget redraws so Today and Overdue roll over to the new day (a job, see [WidgetJobs]). */
object Midnight {
    /** A few seconds past midnight, so the clock has certainly crossed it. */
    private const val SLACK_MS = 5_000L

    /** The next local midnight after [nowMs] in [zone] (daylight-saving aware), plus a little slack. */
    fun nextAfter(nowMs: Long, zone: ZoneId): Long {
        val today = Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate()
        return today.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli() + SLACK_MS
    }

    /** How long from [nowMs] until [nextAfter]: the minimum latency of the midnight job. */
    fun delayFrom(nowMs: Long, zone: ZoneId): Long = nextAfter(nowMs, zone) - nowMs
}
