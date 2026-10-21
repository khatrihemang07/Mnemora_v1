package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TickReplyTest {
    @Test
    fun readsACompletion() {
        val reply = ReplyParser.parseTick(
            """{"ok":{"kind":"completed","message":"Completed “Call mum” · next Wed 28 Oct","token":"{\"reopen\":\"x\",\"notes\":null}"}}""",
        )
        reply as TickReply.Completed
        assertEquals("Completed “Call mum” · next Wed 28 Oct", reply.message)
        assertEquals("""{"reopen":"x","notes":null}""", reply.token)
    }

    @Test
    fun readsTheOpenStepsQuestion() {
        val reply = ReplyParser.parseTick(
            """{"ok":{"kind":"open_steps","task_id":"t1","title":"Pack","count":2}}""",
        )
        assertEquals(TickReply.OpenSteps("t1", "Pack", 2), reply)
    }

    @Test
    fun anythingElseIsAFailure() {
        for (json in listOf(null, "", "nope", """{"err":{"code":"not_found","message":"x"}}""", """{"status":"needs_update"}""", """{"ok":{"kind":"?"}}""")) {
            assertEquals(json, TickReply.Failed, ReplyParser.parseTick(json))
        }
    }

    @Test
    fun readsWhetherAnUndoWorked() {
        assertTrue(ReplyParser.parseUndo("""{"ok":{"undone":true}}"""))
        assertFalse(ReplyParser.parseUndo("""{"err":{"code":"validation","message":"x"}}"""))
        assertFalse(ReplyParser.parseUndo(null))
        assertFalse(ReplyParser.parseUndo("junk"))
    }
}
