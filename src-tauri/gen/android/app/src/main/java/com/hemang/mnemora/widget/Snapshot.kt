package com.hemang.mnemora.widget

import org.json.JSONArray
import org.json.JSONObject

/** How a piece of text is coloured: the theme's usual colour, or red for something late. */
enum class Tone { NORMAL, OVERDUE }

/** The header's count: "3 left", or a red "2 overdue". */
data class HeaderCount(val text: String, val tone: Tone)

/** One Task row, with every word already final (the Rust side words it). */
data class TaskRow(
    val id: String,
    val title: String,
    /** "p1", "p2", "p3" or "none". */
    val ring: String,
    val overdue: Boolean,
    val time: String?,
    val day: String?,
    val list: String?,
)

data class Group(val heading: String?, val tone: Tone, val rows: List<TaskRow>)

data class EmptyState(val title: String, val line: String)

/** A view as the Todo widget draws it. */
data class Snapshot(
    val view: String,
    val title: String,
    val count: HeaderCount?,
    val groups: List<Group>,
    val empty: EmptyState?,
)

/** What the bridge answered. */
sealed interface Reply {
    data class Ok(val snapshot: Snapshot) : Reply

    /** The app has never run here: there is no database. */
    data object NotSetUp : Reply

    /** The app was updated and its migrations have not run yet. */
    data object NeedsUpdate : Reply

    /** Anything else: a bridge error, an unreadable reply, no library. */
    data object Failed : Reply
}

/** Reads the bridge's JSON reply. Never throws: anything unreadable is [Reply.Failed]. */
object ReplyParser {
    fun parse(json: String?): Reply {
        if (json == null) return Reply.Failed
        return try {
            val root = JSONObject(json)
            when {
                root.has("ok") -> Reply.Ok(snapshot(root.getJSONObject("ok")))
                root.optString("status") == "not_set_up" -> Reply.NotSetUp
                root.optString("status") == "needs_update" -> Reply.NeedsUpdate
                else -> Reply.Failed
            }
        } catch (e: Exception) {
            Reply.Failed
        }
    }

    private fun snapshot(o: JSONObject) = Snapshot(
        view = o.getString("view"),
        title = o.getString("title"),
        count = o.optJSONObject("count")?.let {
            HeaderCount(it.getString("text"), tone(it.getString("tone")))
        },
        groups = list(o.getJSONArray("groups")) { group(it) },
        empty = o.optJSONObject("empty")?.let {
            EmptyState(it.getString("title"), it.getString("line"))
        },
    )

    private fun group(o: JSONObject) = Group(
        heading = o.optString("heading").ifEmpty { null },
        tone = tone(o.getString("tone")),
        rows = list(o.getJSONArray("rows")) { row(it) },
    )

    private fun row(o: JSONObject) = TaskRow(
        id = o.getString("id"),
        title = o.getString("title"),
        ring = o.getString("ring"),
        overdue = o.getBoolean("overdue"),
        time = text(o, "time"),
        day = text(o, "day"),
        list = text(o, "list"),
    )

    /** A string field that may be null in the JSON. */
    private fun text(o: JSONObject, key: String): String? =
        if (o.isNull(key)) null else o.getString(key)

    private fun tone(name: String) = if (name == "overdue") Tone.OVERDUE else Tone.NORMAL

    private fun <T> list(array: JSONArray, read: (JSONObject) -> T): List<T> =
        (0 until array.length()).map { read(array.getJSONObject(it)) }
}
