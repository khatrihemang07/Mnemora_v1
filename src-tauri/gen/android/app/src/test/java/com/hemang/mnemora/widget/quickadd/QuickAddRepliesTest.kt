package com.hemang.mnemora.widget.quickadd

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.ZoneId

class QuickAddRepliesTest {
    private val zone = ZoneId.of("UTC")

    @Test
    fun readsTheContext() {
        val json = """{"ok":{"placeholder":"Quick add to 🏠 Home","place":{"date":null,"list_id":"h"},
            "lists":[{"id":"h","name":"Home","emoji":"🏠","label":"🏠 Home"},{"id":"w","name":"Work","emoji":null,"label":"Work"}],
            "parse_dates":true,"week_start":"mon","time_format":"24h"}}"""
        val got = (QuickAddReplies.context(json) as QuickResult.Ok).value
        assertEquals("Quick add to 🏠 Home", got.placeholder)
        assertEquals(listOf("🏠 Home", "Work"), got.lists.map { it.label })
        assertNull(got.lists[1].emoji)
        assertTrue(got.parseDates)
    }

    @Test
    fun readsAReadingWithItsChips() {
        val json = """{"ok":{
            "spans":[{"key":"date:tomorrow:1","kind":"date","start":10,"end":18,"word":"tomorrow","meaning":"Thu 22 Oct","new_list":false}],
            "alternatives":[{"key":"date:fri:1","kind":"date","start":22,"end":25,"word":"fri","meaning":"Fri 23 Oct","new_list":false,"reason":"conflict"}],
            "ignored":[],
            "fields":{"date":"2026-10-22","time":null,"repeat":null,"priority":2,"list_id":null,"new_list":"Garage","from_text":["date"]},
            "list_label":"Garage","list_is_new":true,"when_label":"Tomorrow"}}"""
        val got = (QuickAddReplies.reading(json) as QuickResult.Ok).value
        assertEquals(MarkedWord("date:tomorrow:1", "date", 10, 18, "tomorrow", "Thu 22 Oct", false), got.spans[0])
        assertEquals("fri", got.alternatives[0].word)
        assertEquals(QuickFields("2026-10-22", null, 2, null, "Garage"), got.fields)
        assertTrue(got.listIsNew)
        assertEquals("Tomorrow", got.whenLabel)
    }

    @Test
    fun aReadRepeatIsFlaggedSoTheWhenChipCanShowTheRepeatIcon() {
        fun reading(repeat: String) = """{"ok":{"spans":[],"alternatives":[],"ignored":[],
            "fields":{"date":"2026-10-22","time":null,"repeat":$repeat,"priority":4,"list_id":null,"new_list":null,"from_text":["repeat"]},
            "list_label":"Inbox","list_is_new":false,"when_label":"Thu 22 Oct"}}"""
        val weekly = (QuickAddReplies.reading(reading("""{"freq":"week","every":1,"days":null,"mode":"fixed"}""")) as QuickResult.Ok).value
        assertTrue(weekly.fields.hasRepeat)
        val none = (QuickAddReplies.reading(reading("null")) as QuickResult.Ok).value
        assertFalse(none.fields.hasRepeat)
    }

    @Test
    fun statusesAndErrorsAreNotReadings() {
        assertEquals(QuickResult.NotSetUp, QuickAddReplies.reading("""{"status":"not_set_up"}"""))
        assertEquals(QuickResult.NeedsUpdate, QuickAddReplies.added("""{"status":"needs_update"}"""))
        assertEquals(
            QuickResult.Failed("A title can be at most 500 characters"),
            QuickAddReplies.added("""{"err":{"code":"validation","message":"A title can be at most 500 characters"}}"""),
        )
        assertEquals(QuickResult.Failed(null), QuickAddReplies.added(null))
        assertEquals(QuickResult.Failed(null), QuickAddReplies.added("not json"))
        assertEquals(QuickResult.Failed(null), QuickAddReplies.context("""{"ok":{"placeholder":1}}"""))
    }

    @Test
    fun readsTheToast() {
        val got = QuickAddReplies.added("""{"ok":{"message":"Added “Call plumber” to Today","task":{}}}""")
        assertEquals("Added “Call plumber” to Today", (got as QuickResult.Ok).value.message)
    }

    @Test
    fun aDraftIsAParseOrAddRequestWithEveryDecision() {
        val draft = QuickDraft(
            view = "list:abc",
            text = "Buy bolts tomorrow",
            notes = "- [ ] M6",
            words = CaptureWords(listOf("date:tomorrow:1"), mapOf("time" to "time:9:1")),
            hand = QuickHand(listTouched = true, listId = null, priority = 1),
        )
        for ((method, json) in listOf(
            "parse" to QuickAddRequests.parse(draft, 0L, zone),
            "add" to QuickAddRequests.add(draft, 0L, zone),
        )) {
            val o = JSONObject(json)
            assertEquals(method, o.getString("method"))
            assertEquals("list:abc", o.getString("view"))
            assertEquals("Buy bolts tomorrow", o.getString("text"))
            assertEquals("- [ ] M6", o.getString("notes"))
            assertEquals("date:tomorrow:1", o.getJSONArray("ignored").getString(0))
            assertEquals("time:9:1", o.getJSONObject("prefer").getString("time"))
            // The Inbox chosen by hand is a present, null List.
            assertTrue(o.getJSONObject("hand").has("list_id"))
            assertTrue(o.getJSONObject("hand").isNull("list_id"))
            assertEquals(1, o.getJSONObject("hand").getInt("priority"))
            assertEquals("1970-01-01", o.getJSONObject("local").getString("today"))
        }
    }

    @Test
    fun anUntouchedChipIsAMissingKey() {
        val hand = JSONObject(QuickAddRequests.parse(QuickDraft("today", "x"), 0L, zone)).getJSONObject("hand")
        assertFalse(hand.has("list_id"))
        assertFalse(hand.has("priority"))
    }

    @Test
    fun theContextRequestNamesTheView() {
        val o = JSONObject(QuickAddRequests.context("upcoming", 0L, zone))
        assertEquals("capture_context", o.getString("method"))
        assertEquals("upcoming", o.getString("view"))
    }
}
