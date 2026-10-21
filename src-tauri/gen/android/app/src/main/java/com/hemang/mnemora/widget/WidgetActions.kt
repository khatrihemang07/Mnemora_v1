package com.hemang.mnemora.widget

import android.content.Context
import android.os.Build
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.util.Log
import java.time.ZoneId

/**
 * What a tap on the Todo widget does to Tasks: tick, Undo. Blocking, so callers run it off the main
 * thread (the receiver's `goAsync`, the steps dialog's own executor).
 */
object WidgetActions {
    private const val TAG = "TodoWidget"

    /**
     * Ticks the Task [id]. [tickSteps] answers its open steps (null asks first). A completion plays
     * the light haptic, raises the Undo bar and redraws every Todo widget, so the row is gone and
     * the other widgets are right too.
     */
    fun complete(context: Context, id: String, tickSteps: Boolean?): TickReply {
        val now = System.currentTimeMillis()
        val reply = ReplyParser.parseTick(
            WidgetBridge.call(context, WidgetRequests.complete(id, tickSteps, now, ZoneId.systemDefault())),
        )
        when (reply) {
            is TickReply.Completed -> {
                val until = UndoBar.expiresAt(System.currentTimeMillis())
                UndoBar.put(context, UndoState(reply.message, reply.token, until))
                lightTick(context)
                WidgetUpdater.redrawNow(context)
                WidgetUpdater.hideUndoAt(context, until)
            }
            // The Task changed or went away since the widget was drawn: show what is true now.
            TickReply.Failed -> WidgetUpdater.redrawNow(context)
            is TickReply.OpenSteps -> Unit
        }
        return reply
    }

    /** Takes back the tick the bar shows, if it is still up. */
    fun undo(context: Context) {
        val state = UndoBar.get(context, System.currentTimeMillis())
        UndoBar.clear(context)
        if (state != null) {
            val worked = ReplyParser.parseUndo(WidgetBridge.call(context, WidgetRequests.undo(state.token)))
            if (!worked) Log.w(TAG, "undo failed")
        }
        WidgetUpdater.redrawNow(context)
    }

    /** A light tick, as in the app. Never fails: a haptic is not worth an error. */
    private fun lightTick(context: Context) {
        try {
            val vibrator: Vibrator = if (Build.VERSION.SDK_INT >= 31) {
                context.getSystemService(VibratorManager::class.java).defaultVibrator
            } else {
                @Suppress("DEPRECATION")
                context.getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
            }
            val effect = VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
            if (Build.VERSION.SDK_INT >= 33) {
                // A receiver is a background process: the system drops touch-usage vibrations from one
                // ("ignored_background"), but lets notification-class feedback through.
                vibrator.vibrate(effect, VibrationAttributes.createForUsage(VibrationAttributes.USAGE_NOTIFICATION))
            } else {
                @Suppress("DEPRECATION")
                vibrator.vibrate(
                    effect,
                    android.media.AudioAttributes.Builder()
                        .setUsage(android.media.AudioAttributes.USAGE_NOTIFICATION)
                        .build(),
                )
            }
        } catch (e: Exception) {
            Log.w(TAG, "haptic failed", e)
        }
    }
}
