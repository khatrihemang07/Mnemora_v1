package com.hemang.mnemora.widget

/**
 * What the Todo widget shows at its current size: the list from 3x2 up, otherwise an add button.
 * The thresholds are in dp and pinned by a unit test; they stand for what a launcher gives 3x2
 * cells (about 180x110 dp).
 */
enum class SizeMode {
    /** The card with the Task list. */
    LIST,

    /** The "Add a task..." pill. */
    PILL,

    /** A plain + at the smallest sizes. */
    PLUS,
    ;

    /** The smallest size, in dp, that a layout for this mode is meant for. */
    data class Breakpoint(val mode: SizeMode, val widthDp: Float, val heightDp: Float)

    companion object {
        const val LIST_MIN_WIDTH_DP = 180f
        const val LIST_MIN_HEIGHT_DP = 110f
        const val PILL_MIN_WIDTH_DP = 100f

        /**
         * The layouts a size-responsive RemoteViews carries, smallest first. Android shows the
         * largest one that fits the widget (and the smallest when none does), so these sizes are the
         * thresholds: the PLUS and PILL heights are only above zero so the system ranks them.
         */
        val BREAKPOINTS: List<Breakpoint> = listOf(
            Breakpoint(PLUS, 0f, 0f),
            Breakpoint(PILL, PILL_MIN_WIDTH_DP, 1f),
            Breakpoint(LIST, LIST_MIN_WIDTH_DP, LIST_MIN_HEIGHT_DP),
        )

        /** The mode for a widget [widthDp] wide and [heightDp] high. */
        fun of(widthDp: Float, heightDp: Float): SizeMode = when {
            widthDp >= LIST_MIN_WIDTH_DP && heightDp >= LIST_MIN_HEIGHT_DP -> LIST
            widthDp >= PILL_MIN_WIDTH_DP -> PILL
            else -> PLUS
        }
    }
}
