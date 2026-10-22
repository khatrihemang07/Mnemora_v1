package com.hemang.mnemora.widget

import android.content.Context

/** The Undo bar's text, what undoes the tick, and when the bar goes away. */
data class UndoState(val lead: String, val suffix: String?, val token: String, val until: Long) {
    /** The same bar with its time starting at [drawnAt], when it first showed rather than when it was asked for. */
    fun startedAt(drawnAt: Long, grace: Long = 0L) = copy(until = UndoBar.expiresAt(drawnAt) + grace)
}

/**
 * The themed Undo bar at the bottom of the Todo widget after a tick. It lives for about four
 * seconds, but the clock decides: every draw shows it only before [UndoState.until], so a process
 * that dies before it can redraw cannot leave it up.
 */
object UndoBar {
    const val DURATION_MS = 4_000L

    /**
     * Extra time when the tick was answered in the open-steps dialog: the launcher sits paused under
     * the dialog and applies the redraw only once it is back, up to about 1.4 s after the draw.
     */
    const val DIALOG_GRACE_MS = 1_500L

    /** When a bar shown at [now] ends. */
    fun expiresAt(now: Long) = now + DURATION_MS

    /** Whether the bar is up at [now]. A clock set far back must not keep it up for hours. */
    fun isShown(until: Long, now: Long) = now < until && until - now <= DURATION_MS + DIALOG_GRACE_MS

    /** Whether a stored bar ending at [until] should be cleared at [now]: nothing stored (0) is not. */
    fun isExpired(until: Long, now: Long) = until != 0L && !isShown(until, now)

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

    /** When the stored bar ends, or 0 when none is stored. */
    fun storedUntil(context: Context): Long = prefs(context).getLong("until", 0L)

    /**
     * Clears the stored bar if its time is up, so its Undo token (which may carry the Task's notes)
     * does not outlive it.
     */
    fun settle(context: Context, now: Long) {
        if (isExpired(storedUntil(context), now)) clear(context)
    }

    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
    }
}
