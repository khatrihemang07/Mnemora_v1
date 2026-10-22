package com.hemang.mnemora.widget

/** The date in the Today header: when it shows and the pattern its TextClock gets. */
object HeaderDate {
    /** The short date the locale spells best ("Thu 22", "22 Thu" ...). */
    const val SKELETON = "EEEd"

    /**
     * Only the Today view carries the date, and only where the layout has the room ([wideEnough]).
     * Not tied to the font scale: nothing redraws the Todo widget when it changes, so the rule would go stale.
     * At large text the name ellipsizes first, as the header rules say.
     */
    fun shows(view: String, wideEnough: Boolean): Boolean = view == "today" && wideEnough

    /** A TextClock pattern for [datePattern] with the "·" separator ahead of it, quoted as a literal. */
    fun pattern(datePattern: String): String = "'·' $datePattern"
}
