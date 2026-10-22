package com.hemang.mnemora.widget

import android.content.Context
import org.json.JSONObject

/** One Todo widget's own settings: what it shows, its theme and its background. */
data class WidgetConfig(
    /** `today`, `upcoming`, `inbox`, `all` or `list:<id>`. */
    val view: String = DEFAULT_VIEW,
    val theme: ThemeChoice = ThemeChoice.APP,
    val background: Background = Background.SOLID,
) {
    fun toJson(): String = JSONObject()
        .put("view", view)
        .put("theme", theme.key)
        .put("background", background.key)
        .toString()

    companion object {
        const val DEFAULT_VIEW = "today"

        private val FIXED_VIEWS = setOf("today", "upcoming", "inbox", "all")

        fun isView(view: String?): Boolean =
            view != null && (view in FIXED_VIEWS || (view.startsWith("list:") && view.length > 5))

        /** Reads stored settings. Never throws: anything missing or unreadable is the default. */
        fun fromJson(json: String?): WidgetConfig {
            if (json == null) return WidgetConfig()
            return try {
                val o = JSONObject(json)
                val view = o.optString("view")
                WidgetConfig(
                    view = if (isView(view)) view else DEFAULT_VIEW,
                    theme = ThemeChoice.fromKey(o.optString("theme")),
                    background = Background.fromKey(o.optString("background")),
                )
            } catch (e: Exception) {
                WidgetConfig()
            }
        }
    }
}

/**
 * Per-widget settings, kept on the Android side keyed by appWidgetId. They stay out of the app's
 * shared settings file on purpose: no write races with the app.
 */
object WidgetConfigStore {
    private const val FILE = "todo_widget_config"

    private fun key(id: Int) = "config_$id"

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** The widget's settings, or the defaults when it has none yet. */
    fun get(context: Context, id: Int): WidgetConfig =
        WidgetConfig.fromJson(prefs(context).getString(key(id), null))

    fun put(context: Context, id: Int, config: WidgetConfig) {
        prefs(context).edit().putString(key(id), config.toJson()).apply()
    }

    /** Forgets the settings of removed widgets. */
    fun remove(context: Context, ids: IntArray) {
        val edit = prefs(context).edit()
        ids.forEach { edit.remove(key(it)) }
        edit.apply()
    }

    /** Drops the settings of every widget not in [liveIds]: a safety net if a removal broadcast was missed. */
    fun retainOnly(context: Context, liveIds: IntArray) {
        val prefs = prefs(context)
        val live = liveIds.map { key(it) }.toSet()
        val stale = prefs.all.keys.filter { it.startsWith("config_") && it !in live }
        if (stale.isEmpty()) return
        val edit = prefs.edit()
        stale.forEach { edit.remove(it) }
        edit.apply()
    }

    /** The app's Settings ▸ Theme as last seen in a reply, so error states can still wear it. */
    fun appTheme(context: Context): String? = prefs(context).getString("app_theme", null)

    fun rememberAppTheme(context: Context, theme: String?) {
        if (theme != null && theme != appTheme(context)) {
            prefs(context).edit().putString("app_theme", theme).apply()
        }
    }

    /** Moves settings when the system restores widgets under new ids. */
    fun move(context: Context, oldIds: IntArray, newIds: IntArray) {
        val prefs = prefs(context)
        val edit = prefs.edit()
        oldIds.zip(newIds).forEach { (old, new) ->
            prefs.getString(key(old), null)?.let { edit.putString(key(new), it) }
            edit.remove(key(old))
        }
        edit.apply()
    }
}
