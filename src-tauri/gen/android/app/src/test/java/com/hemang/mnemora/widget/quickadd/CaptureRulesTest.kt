package com.hemang.mnemora.widget.quickadd

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Runs `src/lib/ipc/fixtures/capture-ui.json`, the cases Vitest runs against the app's TypeScript
 * helpers (`capture-ui.test.ts`), against the Kotlin port.
 */
class CaptureRulesTest {
    private val fixture: JSONObject by lazy {
        // Gradle runs unit tests in the app module: .../src-tauri/gen/android/app.
        JSONObject(File("../../../../src/lib/ipc/fixtures/capture-ui.json").readText())
    }

    private fun cases(name: String): List<JSONObject> {
        val array = fixture.getJSONArray(name)
        assertTrue("fixture $name is empty", array.length() > 0)
        return (0 until array.length()).map { array.getJSONObject(it) }
    }

    private fun strings(a: JSONArray) = (0 until a.length()).map { a.getString(it) }

    private fun words(o: JSONObject) = CaptureWords(
        strings(o.getJSONArray("ignored")),
        o.getJSONObject("prefer").let { p -> p.keys().asSequence().associateWith { p.getString(it) } },
    )

    private fun refs(a: JSONArray) =
        (0 until a.length()).map { a.getJSONObject(it) }.map { WordRef(it.getString("kind"), it.getString("key")) }

    private fun optString(o: JSONObject, key: String): String? = if (o.isNull(key)) null else o.getString(key)

    private fun suggestion(o: JSONObject): Suggestion =
        if (o.getString("kind") == "new") {
            Suggestion.New(o.getString("name"))
        } else {
            Suggestion.Existing(o.getString("id"), o.getString("name"), optString(o, "emoji"))
        }

    @Test
    fun ignoreWord() = cases("ignore_word").forEach {
        assertEquals(it.getString("name"), words(it.getJSONObject("expected")), CaptureRules.ignoreWord(words(it.getJSONObject("words")), it.getString("key")))
    }

    @Test
    fun recogniseAgain() = cases("recognise_again").forEach {
        assertEquals(it.getString("name"), words(it.getJSONObject("expected")), CaptureRules.recogniseAgain(words(it.getJSONObject("words")), it.getString("key")))
    }

    @Test
    fun preferWord() = cases("prefer_word").forEach {
        val span = it.getJSONObject("span")
        val got = CaptureRules.preferWord(words(it.getJSONObject("words")), WordRef(span.getString("kind"), span.getString("key")))
        assertEquals(it.getString("name"), words(it.getJSONObject("expected")), got)
    }

    @Test
    fun ignoreKinds() = cases("ignore_kinds").forEach {
        val found = refs(it.getJSONArray("spans")) + refs(it.getJSONArray("alternatives"))
        val got = CaptureRules.ignoreKinds(words(it.getJSONObject("words")), found, strings(it.getJSONArray("kinds")))
        assertEquals(it.getString("name"), words(it.getJSONObject("expected")), got)
    }

    @Test
    fun pruneWords() = cases("prune_words").forEach {
        val got = CaptureRules.pruneWords(words(it.getJSONObject("words")), strings(it.getJSONArray("known")))
        assertEquals(it.getString("name"), words(it.getJSONObject("expected")), got)
    }

    @Test
    fun backspaceTarget() = cases("backspace_target").forEach {
        val spans = it.getJSONArray("spans").let { a -> (0 until a.length()).map { i -> a.getJSONObject(i) } }
            .map { s -> SpanEnd(s.getString("key"), s.getInt("end")) }
        val got = CaptureRules.backspaceTarget(
            it.getString("text"), it.getInt("selection_start"), it.getInt("selection_end"), spans,
            it.getBoolean("just_unrecognised"),
        )
        assertEquals(it.getString("name"), optString(it, "expected"), got)
    }

    @Test
    fun foldTag() = cases("fold_tag").forEach {
        assertEquals(it.getString("input"), it.getString("expected"), CaptureRules.foldTag(it.getString("input")))
    }

    @Test
    fun tagOf() = cases("tag_of").forEach {
        assertEquals(it.getString("input"), it.getString("expected"), CaptureRules.tagOf(it.getString("input")))
    }

    @Test
    fun hashQuery() = cases("hash_query").forEach {
        val got = CaptureRules.hashQuery(it.getString("text"), it.getInt("caret"))
        if (it.isNull("expected")) {
            assertNull(it.toString(), got)
        } else {
            val want = it.getJSONObject("expected")
            assertEquals(it.toString(), HashQuery(want.getInt("start"), want.getString("query")), got)
        }
    }

    @Test
    fun listSuggestions() = cases("list_suggestions").forEach {
        val lists = it.getJSONArray("lists").let { a -> (0 until a.length()).map { i -> a.getJSONObject(i) } }
            .map { l -> SuggestList(l.getString("id"), l.getString("name"), optString(l, "emoji")) }
        val want = it.getJSONArray("expected").let { a -> (0 until a.length()).map { i -> suggestion(a.getJSONObject(i)) } }
        assertEquals(it.getString("name"), want, CaptureRules.listSuggestions(it.getString("query"), lists))
    }

    @Test
    fun applySuggestion() = cases("apply_suggestion").forEach {
        val hash = it.getJSONObject("hash").let { h -> HashQuery(h.getInt("start"), h.getString("query")) }
        val got = CaptureRules.applySuggestion(it.getString("text"), hash, suggestion(it.getJSONObject("suggestion")))
        val want = it.getJSONObject("expected")
        assertEquals(it.getString("name"), Pair(want.getString("text"), want.getInt("caret")), got)
    }

    @Test
    fun markSegments() = cases("mark_segments").forEach {
        val spans = it.getJSONArray("spans").let { a -> (0 until a.length()).map { i -> a.getJSONObject(i) } }
            .map { s -> TextSpan(s.getInt("start"), s.getInt("end")) }
        val want = it.getJSONArray("expected").let { a -> (0 until a.length()).map { i -> a.getJSONObject(i) } }
            .map { s -> Segment(s.getString("text"), s.getBoolean("mark")) }
        assertEquals(it.getString("name"), want, CaptureRules.markSegments(it.getString("text"), spans))
    }

    @Test
    fun pasteParts() = cases("paste_parts").forEach {
        val got = CaptureRules.pasteParts(it.getString("pasted"))
        if (it.isNull("expected")) {
            assertNull(it.getString("name"), got)
        } else {
            val want = it.getJSONObject("expected")
            assertEquals(it.getString("name"), PasteParts(want.getString("title"), want.getString("notes")), got)
        }
    }
}
