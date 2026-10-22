package com.hemang.mnemora.widget

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** Builds the links in `src/lib/ipc/fixtures/todo-links.json`, which Vitest parses back. */
class WidgetLinksTest {
    private fun fixture(): JSONObject {
        // Gradle runs unit tests in the app module: .../src-tauri/gen/android/app.
        val path = "../../../../src/lib/ipc/fixtures/todo-links.json"
        return JSONObject(File(path).readText())
    }

    @Test
    fun buildsEveryCanonicalLink() {
        val cases = fixture().getJSONArray("canonical")
        assertTrue(cases.length() > 0)
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val task = if (case.isNull("task")) null else case.getString("task")
            assertEquals(case.getString("name"), case.getString("url"), WidgetLinks.build(case.getString("view"), task))
        }
    }

    @Test
    fun everyBuiltLinkIsOneTheWidgetAccepts() {
        val cases = fixture().getJSONArray("canonical")
        for (i in 0 until cases.length()) {
            assertTrue(WidgetLinks.isLink(cases.getJSONObject(i).getString("url")))
        }
    }

    @Test
    fun onlyTodoLinksAreAccepted() {
        assertTrue(WidgetLinks.isLink("mnemora://todo"))
        assertFalse(WidgetLinks.isLink("mnemora://todox?view=today"))
        assertFalse(WidgetLinks.isLink("mnemora://journal"))
        assertFalse(WidgetLinks.isLink("https://todo?view=today"))
        assertFalse(WidgetLinks.isLink(null))
    }
}
