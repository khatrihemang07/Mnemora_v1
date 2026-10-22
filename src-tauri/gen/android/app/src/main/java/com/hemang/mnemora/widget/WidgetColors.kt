package com.hemang.mnemora.widget

import android.content.Context
import android.content.res.ColorStateList
import android.widget.RemoteViews
import androidx.annotation.RequiresApi

/** One look with its palette: what a colour is picked from. */
class Shade(val palette: Palette, val look: Look)

/**
 * Applies colours to a widget's RemoteViews. A widget that follows the phone ("Same as app" while
 * the app's theme is System) is given both a day and a night colour for each call, so the launcher
 * switches them itself when night mode flips, with no redraw (API 31 `setColorInt` and
 * `setColorStateList` with two values). Every other widget has one fixed look.
 */
@RequiresApi(31)
class WidgetColors private constructor(private val day: Shade, private val night: Shade?, phoneNight: Boolean) {
    /** The look now: the only one, or whichever the phone is in. Layout choices that cannot flip use it. */
    val current: Shade = if (night != null && phoneNight) night else day

    /** Sets a text colour. [pick] reads it from the shade it is called on. */
    fun text(views: RemoteViews, id: Int, pick: Shade.() -> Int) {
        if (night == null) views.setTextColor(id, day.pick()) else views.setColorInt(id, "setTextColor", day.pick(), night.pick())
    }

    /** Sets a tint list through [method] (`setImageTintList`, `setBackgroundTintList`). */
    fun tint(views: RemoteViews, id: Int, method: String, pick: Shade.() -> Int) {
        if (night == null) {
            views.setColorStateList(id, method, ColorStateList.valueOf(day.pick()))
        } else {
            views.setColorStateList(id, method, ColorStateList.valueOf(day.pick()), ColorStateList.valueOf(night.pick()))
        }
    }

    companion object {
        /** The colours for a widget showing [reply] with [config]'s Theme and Background. */
        fun of(context: Context, appTheme: String?, config: WidgetConfig, phoneNight: Boolean): WidgetColors {
            fun shade(look: Look) = Shade(Palette.from(context, look, config.background), look)
            return if (WidgetTheme.followsPhone(config.theme, appTheme)) {
                WidgetColors(shade(Look.LIGHT), shade(Look.DARK), phoneNight)
            } else {
                WidgetColors(shade(WidgetTheme.resolve(config.theme, appTheme, phoneNight)), null, phoneNight)
            }
        }
    }
}
