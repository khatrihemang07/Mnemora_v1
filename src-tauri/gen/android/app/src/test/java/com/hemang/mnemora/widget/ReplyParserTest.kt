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
    fun readsTheAppsThemeFromASnapshot() {
        val json = """{"ok":{"view":"all","app_theme":"oled","title":"All","count":null,"groups":[],
            "empty":{"title":"No open Tasks","line":"Everything is done"}}}"""
        assertEquals("oled", (ReplyParser.parse(json) as Reply.Ok).snapshot.appTheme)
        assertNull((ReplyParser.parse(ok) as Reply.Ok).snapshot.appTheme)
    }

    @Test
    fun aGroupWithoutAHeadingHasNoHeadingText() {
        val json = """{"ok":{"view":"inbox","title":"Inbox","count":{"text":"1","tone":"normal"},"app_theme":null,
            "groups":[{"heading":null,"tone":"normal","rows":[
              {"id":"a","title":"Idea","ring":"none","overdue":false,"time":null,"day":null,"list":null}]}],
            "empty":null}}"""
        val snapshot = (ReplyParser.parse(json) as Reply.Ok).snapshot
        assertNull(snapshot.groups[0].heading)
        assertNull(snapshot.appTheme)
        assertEquals(listOf<Item>(Item.Task(snapshot.groups[0].rows[0])), RowItems.flatten(snapshot.groups))
    }

    @Test
    fun aDeletedListIsReportedAsSuch() {
        val err = """{"err":{"code":"not_found","message":"That List no longer exists"}}"""
        assertEquals(Reply.ListMissing, ReplyParser.parse(err))
    }

    @Test
    fun readsTheListsForSetup() {
        val json = """{"ok":{"app_theme":"dark","lists":[
            {"id":"a","name":"Home","emoji":"🏠","label":"🏠 Home"},
            {"id":"b","name":"Work","emoji":null,"label":"Work"}]}}"""
        val choices = ReplyParser.parseLists(json)!!
        assertEquals("dark", choices.appTheme)
        assertEquals(listOf(ListChoice("a", "Home", "🏠", "🏠 Home"), ListChoice("b", "Work", null, "Work")), choices.lists)
    }

    @Test
    fun noListsWhenTheBridgeCannotAnswer() {
        for (bad in listOf(null, "", "not json", """{"status":"not_set_up"}""", """{"err":{"code":"x"}}""", """{"ok":{}}""")) {
            assertNull("$bad", ReplyParser.parseLists(bad))
        }
    }

    @Test
    fun anythingElseIsFailed() {
        val err = """{"err":{"code":"database","message":"Database error"}}"""
        for (bad in listOf(err, "", "not json", "[]", "{}", """{"ok":{"view":"today"}}""", null)) {
            assertEquals("$bad", Reply.Failed, ReplyParser.parse(bad))
        }
    }
}
