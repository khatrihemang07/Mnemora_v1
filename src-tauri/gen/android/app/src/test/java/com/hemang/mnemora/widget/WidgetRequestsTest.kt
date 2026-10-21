package com.hemang.mnemora.widget

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneId
import java.time.ZonedDateTime

class WidgetRequestsTest {
    private fun ms(text: String) = ZonedDateTime.parse(text).toInstant().toEpochMilli()

    @Test
    fun localCarriesTheDevicesDateTimeAndOffset() {
        val local = WidgetRequests.local(ms("2026-10-21T14:05:30+05:30"), ZoneId.of("Asia/Kolkata"))
        assertEquals("2026-10-21", local.getString("today"))
        assertEquals("14:05", local.getString("now"))
        assertEquals(330, local.getInt("offset_min"))
    }

    @Test
    fun theDateIsTheLocalOneNotUtc() {
        // 23:30 in New York on the 20th is already the 21st in UTC.
        val local = WidgetRequests.local(ms("2026-10-20T23:30:00-04:00"), ZoneId.of("America/New_York"))
        assertEquals("2026-10-20", local.getString("today"))
        assertEquals("23:30", local.getString("now"))
        assertEquals(-240, local.getInt("offset_min"))
    }

    @Test
    fun snapshotRequestNamesTheMethodAndView() {
        val json = JSONObject(WidgetRequests.snapshot("today", ms("2026-10-21T09:00:00Z"), ZoneId.of("UTC")))
        assertEquals("snapshot", json.getString("method"))
        assertEquals("today", json.getString("view"))
        assertEquals("2026-10-21", json.getJSONObject("local").getString("today"))
    }

    @Test
    fun completeAsksFirstWhenNoAnswerToOpenStepsIsGiven() {
        val json = JSONObject(WidgetRequests.complete("t1", null, ms("2026-10-21T09:00:00Z"), ZoneId.of("UTC")))
        assertEquals("complete", json.getString("method"))
        assertEquals("t1", json.getString("id"))
        assertTrue(json.isNull("tick_steps") || !json.has("tick_steps"))
        assertEquals("2026-10-21", json.getJSONObject("local").getString("today"))
    }

    @Test
    fun completeCarriesTheAnswerToOpenSteps() {
        val all = JSONObject(WidgetRequests.complete("t1", true, 0L, ZoneId.of("UTC")))
        assertEquals(true, all.getBoolean("tick_steps"))
        val asIs = JSONObject(WidgetRequests.complete("t1", false, 0L, ZoneId.of("UTC")))
        assertEquals(false, asIs.getBoolean("tick_steps"))
    }

    @Test
    fun undoCarriesTheTokenUntouched() {
        val token = """{"reopen":"x","notes":"- [ ] a\n"}"""
        val json = JSONObject(WidgetRequests.undo(token))
        assertEquals("undo", json.getString("method"))
        assertEquals(token, json.getString("token"))
    }
}
