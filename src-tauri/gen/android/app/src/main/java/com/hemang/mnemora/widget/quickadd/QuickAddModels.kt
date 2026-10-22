package com.hemang.mnemora.widget.quickadd

import com.hemang.mnemora.widget.WidgetRequests
import org.json.JSONArray
import org.json.JSONObject
import java.time.ZoneId

/** A List the sheet can pick, with its label (emoji and name) already worded by Rust. */
data class PickList(val id: String, val name: String, val emoji: String?, val label: String)

/** What the sheet needs on opening (the bridge's `capture_context`). */
data class QuickContext(
    /** The field's hint: "Quick add, due today". */
    val placeholder: String,
    /** The Lists, in the sidebar's order. */
    val lists: List<PickList>,
    /** "Recognise dates in titles": when off, nothing is read or highlighted. */
    val parseDates: Boolean,
)

/** A recognised word with its meaning in words, ready for a chip. */
data class MarkedWord(
    val key: String,
    val kind: String,
    val start: Int,
    val end: Int,
    val word: String,
    val meaning: String,
    val newList: Boolean,
)

/** The fields the Task would get. Only what the sheet shows. */
data class QuickFields(val date: String?, val time: String?, val priority: Int, val listId: String?, val newList: String?)

/** What the sheet shows for the text so far (the bridge's `parse`). */
data class Reading(
    val spans: List<MarkedWord>,
    val alternatives: List<MarkedWord>,
    val ignored: List<MarkedWord>,
    val fields: QuickFields,
    val listLabel: String,
    val listIsNew: Boolean,
    val whenLabel: String,
)

/** A Task that was added, and the toast's words. */
data class Added(val message: String)

/** What the bridge answered, for the sheet. */
sealed interface QuickResult<out T> {
    data class Ok<T>(val value: T) : QuickResult<T>

    /** The app has never run here. */
    data object NotSetUp : QuickResult<Nothing>

    /** The app was updated and has not migrated yet. */
    data object NeedsUpdate : QuickResult<Nothing>

    /** Anything else; [message] is Rust's own words when it gave any. */
    data class Failed(val message: String?) : QuickResult<Nothing>
}

/** The chips used by hand: the List and the Priority (there is no When picker). */
data class QuickHand(
    /** Whether the List chip was used; [listId] null then means the Inbox. */
    val listTouched: Boolean = false,
    val listId: String? = null,
    val priority: Int? = null,
)

/** Everything a `parse` or `add` request is made of. */
data class QuickDraft(
    val view: String,
    val text: String,
    val notes: String = "",
    val words: CaptureWords = CaptureWords(),
    val hand: QuickHand = QuickHand(),
)

object QuickAddRequests {
    fun context(view: String, epochMs: Long, zone: ZoneId): String =
        JSONObject()
            .put("method", "capture_context")
            .put("view", view)
            .put("local", WidgetRequests.local(epochMs, zone))
            .toString()

    fun parse(draft: QuickDraft, epochMs: Long, zone: ZoneId): String = draft(draft, "parse", epochMs, zone)

    fun add(draft: QuickDraft, epochMs: Long, zone: ZoneId): String = draft(draft, "add", epochMs, zone)

    private fun draft(d: QuickDraft, method: String, epochMs: Long, zone: ZoneId): String {
        val hand = JSONObject()
        // A key is there only for a chip that was used; null is the Inbox.
        if (d.hand.listTouched) hand.put("list_id", d.hand.listId ?: JSONObject.NULL)
        d.hand.priority?.let { hand.put("priority", it) }
        return JSONObject()
            .put("method", method)
            .put("view", d.view)
            .put("text", d.text)
            .put("notes", d.notes)
            .put("ignored", JSONArray(d.words.ignored))
            .put("prefer", JSONObject(d.words.prefer))
            .put("hand", hand)
            .put("local", WidgetRequests.local(epochMs, zone))
            .toString()
    }
}

/** Reads the bridge's replies for the sheet. Never throws: anything unreadable is [QuickResult.Failed]. */
object QuickAddReplies {
    fun context(json: String?): QuickResult<QuickContext> = read(json) { ok ->
        QuickContext(
            placeholder = ok.getString("placeholder"),
            lists = list(ok.getJSONArray("lists")) {
                PickList(it.getString("id"), it.getString("name"), text(it, "emoji"), it.getString("label"))
            },
            parseDates = ok.getBoolean("parse_dates"),
        )
    }

    fun reading(json: String?): QuickResult<Reading> = read(json) { ok ->
        val fields = ok.getJSONObject("fields")
        Reading(
            spans = list(ok.getJSONArray("spans")) { word(it) },
            alternatives = list(ok.getJSONArray("alternatives")) { word(it) },
            ignored = list(ok.getJSONArray("ignored")) { word(it) },
            fields = QuickFields(
                date = text(fields, "date"),
                time = text(fields, "time"),
                priority = fields.getInt("priority"),
                listId = text(fields, "list_id"),
                newList = text(fields, "new_list"),
            ),
            listLabel = ok.getString("list_label"),
            listIsNew = ok.getBoolean("list_is_new"),
            whenLabel = ok.getString("when_label"),
        )
    }

    fun added(json: String?): QuickResult<Added> = read(json) { Added(it.getString("message")) }

    private fun <T> read(json: String?, parse: (JSONObject) -> T): QuickResult<T> {
        if (json == null) return QuickResult.Failed(null)
        return try {
            val root = JSONObject(json)
            when {
                root.has("ok") -> QuickResult.Ok(parse(root.getJSONObject("ok")))
                root.optString("status") == "not_set_up" -> QuickResult.NotSetUp
                root.optString("status") == "needs_update" -> QuickResult.NeedsUpdate
                else -> QuickResult.Failed(root.optJSONObject("err")?.optString("message")?.ifEmpty { null })
            }
        } catch (e: Exception) {
            QuickResult.Failed(null)
        }
    }

    private fun word(o: JSONObject) = MarkedWord(
        key = o.getString("key"),
        kind = o.getString("kind"),
        start = o.getInt("start"),
        end = o.getInt("end"),
        word = o.getString("word"),
        meaning = o.getString("meaning"),
        newList = o.getBoolean("new_list"),
    )

    private fun text(o: JSONObject, key: String): String? = if (o.isNull(key)) null else o.getString(key)

    private fun <T> list(array: JSONArray, read: (JSONObject) -> T): List<T> =
        (0 until array.length()).map { read(array.getJSONObject(it)) }
}
