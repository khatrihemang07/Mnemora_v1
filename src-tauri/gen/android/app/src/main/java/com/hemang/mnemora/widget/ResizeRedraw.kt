package com.hemang.mnemora.widget

/**
 * Whether the launcher's "options changed" call needs a redraw. The responsive RemoteViews already
 * pick the layout for each size, so a resize is not a reason to read and draw again: the ticks of a
 * drag arrive in dozens. A process that has not drawn lately (just started, or killed since) does
 * draw, which is also what a forced `wm density` change relies on.
 */
object ResizeRedraw {
    /** A draw this recent already shows what a resize would. */
    const val FRESH_MS = 60_000L

    fun due(lastDrawAt: Long, now: Long): Boolean = lastDrawAt == 0L || now - lastDrawAt !in 0..FRESH_MS
}
