package com.hemang.mnemora.widget

import android.content.Context
import androidx.annotation.ColorInt
import androidx.core.content.ContextCompat
import com.hemang.mnemora.R

/** The Todo widget's colours, hand-converted from the theme tokens (`values/widget_themes.xml`). */
data class Palette(
    @ColorInt val background: Int,
    @ColorInt val surface2: Int,
    @ColorInt val ink: Int,
    @ColorInt val muted: Int,
    @ColorInt val faint: Int,
    @ColorInt val danger: Int,
    @ColorInt val warning: Int,
    @ColorInt val accent: Int,
    @ColorInt val todo: Int,
    @ColorInt val onTodo: Int = 0xFFFFFFFF.toInt(),
) {
    /** The diamond's edge colour for a Priority ring; an Overdue Task with none gets a reddish edge. */
    @ColorInt
    fun ringEdge(ring: String, overdue: Boolean): Int = when (ring) {
        "p1" -> danger
        "p2" -> warning
        "p3" -> accent
        else -> if (overdue) blend(danger, faint, 0.7f) else faint
    }

    /** The diamond's soft fill for a Priority ring, or 0 for none. */
    @ColorInt
    fun ringFill(ring: String): Int = when (ring) {
        "p1" -> withAlpha(danger, 0.12f)
        "p2" -> withAlpha(warning, 0.14f)
        "p3" -> withAlpha(accent, 0.14f)
        else -> 0
    }

    /** With no card the secondary text has no surface to contrast with, so it steps up one level. */
    fun forWallpaper(): Palette = copy(
        background = 0,
        muted = blend(ink, muted, 0.5f),
        faint = muted,
    )

    companion object {
        /** The palette for [look], drawn on [background]. */
        fun from(context: Context, look: Look, background: Background = Background.SOLID): Palette {
            fun c(id: Int) = ContextCompat.getColor(context, id)
            val palette = when (look) {
                Look.LIGHT -> Palette(
                    c(R.color.widget_light_bg), c(R.color.widget_light_surface2), c(R.color.widget_light_ink),
                    c(R.color.widget_light_muted), c(R.color.widget_light_faint), c(R.color.widget_light_danger),
                    c(R.color.widget_light_warning), c(R.color.widget_light_accent), c(R.color.widget_light_todo),
                    c(R.color.widget_light_on_todo),
                )
                Look.DARK -> Palette(
                    c(R.color.widget_dark_bg), c(R.color.widget_dark_surface2), c(R.color.widget_dark_ink),
                    c(R.color.widget_dark_muted), c(R.color.widget_dark_faint), c(R.color.widget_dark_danger),
                    c(R.color.widget_dark_warning), c(R.color.widget_dark_accent), c(R.color.widget_dark_todo),
                    c(R.color.widget_dark_on_todo),
                )
                Look.TRUE_BLACK -> Palette(
                    c(R.color.widget_oled_bg), c(R.color.widget_oled_surface2), c(R.color.widget_oled_ink),
                    c(R.color.widget_oled_muted), c(R.color.widget_oled_faint), c(R.color.widget_oled_danger),
                    c(R.color.widget_oled_warning), c(R.color.widget_oled_accent), c(R.color.widget_oled_todo),
                    c(R.color.widget_oled_on_todo),
                )
                Look.MATERIAL_YOU -> Palette(
                    c(R.color.widget_you_bg), c(R.color.widget_you_surface2), c(R.color.widget_you_ink),
                    c(R.color.widget_you_muted), c(R.color.widget_you_faint), c(R.color.widget_you_danger),
                    c(R.color.widget_you_warning), c(R.color.widget_you_accent), c(R.color.widget_you_todo),
                    c(R.color.widget_you_on_todo),
                )
                Look.MATERIAL_YOU_DARK -> Palette(
                    c(R.color.widget_you_dark_bg), c(R.color.widget_you_dark_surface2), c(R.color.widget_you_dark_ink),
                    c(R.color.widget_you_dark_muted), c(R.color.widget_you_dark_faint), c(R.color.widget_you_dark_danger),
                    c(R.color.widget_you_dark_warning), c(R.color.widget_you_dark_accent), c(R.color.widget_you_dark_todo),
                    c(R.color.widget_you_dark_on_todo),
                )
            }
            // No background: the same text colours straight on the wallpaper (a soft shadow or glow
            // in the layout keeps them readable), so the theme still means light or dark text.
            return if (background == Background.NONE) palette.forWallpaper() else palette
        }

        /** The faint wash behind a widget with no background: black on dark looks, white on light ones. */
        @ColorInt
        fun scrim(look: Look): Int =
            if (WidgetTheme.isDark(look)) withAlpha(0xFF000000.toInt(), 0.5f) else withAlpha(0xFFFFFFFF.toInt(), 0.55f)

        @ColorInt
        fun withAlpha(@ColorInt color: Int, alpha: Float): Int =
            (color and 0x00FFFFFF) or ((alpha * 255).toInt().coerceIn(0, 255) shl 24)

        /** [a] weighted [weightA] against [b], both opaque. */
        @ColorInt
        fun blend(@ColorInt a: Int, @ColorInt b: Int, weightA: Float): Int {
            fun mix(shift: Int): Int {
                val x = (a shr shift) and 0xFF
                val y = (b shr shift) and 0xFF
                return (x * weightA + y * (1 - weightA)).toInt().coerceIn(0, 255)
            }
            return (0xFF shl 24) or (mix(16) shl 16) or (mix(8) shl 8) or mix(0)
        }
    }
}
