package com.hemang.mnemora.widget

/** What a Todo widget's Theme setting can be. */
enum class ThemeChoice(val key: String) {
    /** Follows Settings ▸ Theme in the app. */
    APP("app"),
    LIGHT("light"),
    DARK("dark"),
    TRUE_BLACK("oled"),
    ;

    companion object {
        /**
         * Reads a stored key; anything unknown is [APP]. That includes the retired Material You keys
         * ("you", "you_dark"), so a widget saved with them follows the app's theme.
         */
        fun fromKey(key: String?): ThemeChoice = values().firstOrNull { it.key == key } ?: APP
    }
}

/** The look a widget is actually drawn in. */
enum class Look { LIGHT, DARK, TRUE_BLACK }

/** What a Todo widget's Background setting can be. */
enum class Background(val key: String, val alpha: Float) {
    /** The theme's opaque surface. */
    SOLID("solid", 1f),

    /** The surface at about 70%: Android cannot blur behind a widget. */
    SEE_THROUGH("see_through", 0.72f),

    /** No surface: text straight on the wallpaper. */
    NONE("none", 0f),
    ;

    companion object {
        /** Reads a stored key; anything unknown is [SOLID]. */
        fun fromKey(key: String?): Background = values().firstOrNull { it.key == key } ?: SOLID
    }
}

object WidgetTheme {
    /**
     * The look for a [choice]. "Same as app" follows the app's Settings ▸ Theme ([appTheme]:
     * `light`, `dark`, `oled` or `system`); on `system`, or when the app's theme is not known
     * (the app has not run yet), the phone's night mode decides.
     */
    fun resolve(choice: ThemeChoice, appTheme: String?, phoneNight: Boolean): Look = when (choice) {
        ThemeChoice.LIGHT -> Look.LIGHT
        ThemeChoice.DARK -> Look.DARK
        ThemeChoice.TRUE_BLACK -> Look.TRUE_BLACK
        ThemeChoice.APP -> when (appTheme) {
            "light" -> Look.LIGHT
            "dark" -> Look.DARK
            "oled" -> Look.TRUE_BLACK
            else -> if (phoneNight) Look.DARK else Look.LIGHT
        }
    }

    /**
     * Whether the look follows the phone's night mode: "Same as app" while the app's own theme is
     * System (or unknown). Such a widget is drawn in both looks, and the launcher picks one itself
     * when night mode flips, with no redraw.
     */
    fun followsPhone(choice: ThemeChoice, appTheme: String?): Boolean =
        choice == ThemeChoice.APP && appTheme !in setOf("light", "dark", "oled")

    /** Whether a look is a dark one (its Priority colours are the lighter set). */
    fun isDark(look: Look): Boolean = look != Look.LIGHT
}
