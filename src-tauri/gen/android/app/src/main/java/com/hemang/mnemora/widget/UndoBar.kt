package com.hemang.mnemora.widget

import android.content.Context

/** The Undo bar's text, what undoes the tick, and when the bar goes away. */
data class UndoState(val lead: String, val suffix: String?, val token: String, val until: Long)

/**
 * The themed Undo bar at the bottom of the Todo widget after a tick. It lives for about four
 * seconds, but the clock decides: every draw shows it only before [UndoState.until], so a process
 * that dies before it can redraw cannot leave it up.
 */
object UndoBar {
    const val DURATION_MS = 4_000L

    /** When a bar shown at [now] ends. */
    fun expiresAt(now: Long) = now + DURATION_MS

    /** Whether the bar is up at [now]. A clock set far back must not keep it up for hours. */
    fun isShown(until: Long, now: Long) = now < until && until - now <= DURATION_MS

    private const val FILE = "todo_widget_undo"

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** Keeps the bar for the next draw; a later tick replaces it, like the app's toast. */
    fun put(context: Context, state: UndoState) {
        prefs(context).edit()
            .putString("lead", state.lead)
            .putString("suffix", state.suffix)
            .putString("token", state.token)
            .putLong("until", state.until)
            .apply()
    }

    /** The bar to show at [now], or none. */
    fun get(context: Context, now: Long): UndoState? {
        val p = prefs(context)
        val lead = p.getString("lead", null) ?: return null
        val token = p.getString("token", null) ?: return null
        val until = p.getLong("until", 0L)
        return if (isShown(until, now)) UndoState(lead, p.getString("suffix", null), token, until) else null
    }

    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
    }
}
