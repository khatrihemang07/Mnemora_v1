package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReplyParserTest {
    private val ok = """
        {"ok":{"view":"today","title":"Today",
         "count":{"text":"2 overdue","tone":"overdue"},
         "groups":[
           {"heading":"Overdue","tone":"overdue","rows":[
             {"id":"a","title":"Renew passport","ring":"p2","overdue":true,"time":null,"day":"Mon","list":null}]},
           {"heading":"Today","tone":"normal","rows":[
             {"id":"b","title":"Call mum","ring":"none","overdue":false,"time":"18:00","day":null,"list":"🏠 Home"}]}],
         "empty":null}}
    """.trimIndent()

    @Test
    fun readsASnapshot() {
        val reply = ReplyParser.parse(ok) as Reply.Ok
        val snapshot = reply.snapshot
        assertEquals("Today", snapshot.title)
        assertEquals(HeaderCount("2 overdue", Tone.OVERDUE), snapshot.count)
        assertEquals(listOf("Overdue", "Today"), snapshot.groups.map { it.heading })
        assertEquals(Tone.OVERDUE, snapshot.groups[0].tone)
        val late = snapshot.groups[0].rows[0]
        assertEquals("Mon", late.day)
        assertNull(late.time)
        assertEquals("p2", late.ring)
        assertTrue(late.overdue)
        val timed = snapshot.groups[1].rows[0]
        assertEquals("18:00", timed.time)
        assertEquals("🏠 Home", timed.list)
        assertNull(snapshot.empty)
    }

    @Test
    fun readsAnEmptyState() {
        val json = """{"ok":{"view":"today","title":"Today","count":null,"groups":[],
            "empty":{"title":"Nothing due today","line":"Next: Dentist, tomorrow 11:00"}}}"""
        val snapshot = (ReplyParser.parse(json) as Reply.Ok).snapshot
        assertNull(snapshot.count)
        assertEquals(EmptyState("Nothing due today", "Next: Dentist, tomorrow 11:00"), snapshot.empty)
    }

    @Test
    fun readsTheStatuses() {
        assertEquals(Reply.NotSetUp, ReplyParser.parse("""{"status":"not_set_up"}"""))
        assertEquals(Reply.NeedsUpdate, ReplyParser.parse("""{"status":"needs_update"}"""))
    }

    @Test
    fun anythingElseIsFailed() {
        val err = """{"err":{"code":"database","message":"Database error"}}"""
        for (bad in listOf(err, "", "not json", "[]", "{}", """{"ok":{"view":"today"}}""", null)) {
            assertEquals("$bad", Reply.Failed, ReplyParser.parse(bad))
        }
    }
}
