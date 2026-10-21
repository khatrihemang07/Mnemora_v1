package com.hemang.mnemora.widget

import android.content.Context
import androidx.annotation.ColorInt
import androidx.core.content.ContextCompat
import com.hemang.mnemora.R

/** The Todo widget's colours, hand-converted from the theme tokens (`values/` light, `values-night/` dark). */
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

    companion object {
        /** The palette for the app's current light or dark look. */
        fun from(context: Context): Palette {
            fun c(id: Int) = ContextCompat.getColor(context, id)
            return Palette(
                background = c(R.color.widget_bg),
                surface2 = c(R.color.widget_surface2),
                ink = c(R.color.widget_ink),
                muted = c(R.color.widget_muted),
                faint = c(R.color.widget_faint),
                danger = c(R.color.widget_danger),
                warning = c(R.color.widget_warning),
                accent = c(R.color.widget_accent),
                todo = c(R.color.widget_todo),
            )
        }

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
