package com.hemang.mnemora.widget

import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.hemang.mnemora.MainActivity
import java.util.concurrent.Executors

/**
 * Taps on the Todo widget that change Tasks: a row's diamond (tick) and the Undo bar. Not exported;
 * only this app's own PendingIntents reach it. The work runs off the main thread under `goAsync`.
 */
class WidgetActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action
        if (action != ACTION_ROW && action != ACTION_UNDO) return
        val app = context.applicationContext
        val pending = goAsync()
        executor.execute {
            try {
                if (action == ACTION_UNDO) WidgetActions.undo(app) else row(app, intent)
            } catch (e: Throwable) {
                Log.w(TAG, "widget action failed", e)
            } finally {
                pending.finish()
            }
        }
    }

    private fun row(context: Context, intent: Intent) {
        when (intent.getStringExtra(EXTRA_KIND)) {
            KIND_TICK -> {
                val id = intent.getStringExtra(EXTRA_TASK_ID) ?: return
                val reply = WidgetActions.complete(context, id, null)
                if (reply is TickReply.OpenSteps) {
                    val widgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
                    context.startActivity(StepsDialogActivity.intent(context, reply, widgetId))
                }
            }
            // Any other part of a row opens the app (what it shows is for the row-tap ticket).
            else -> context.startActivity(
                Intent(context, MainActivity::class.java)
                    .setAction(Intent.ACTION_MAIN)
                    .addCategory(Intent.CATEGORY_LAUNCHER)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }

    companion object {
        private const val TAG = "TodoWidget"
        const val ACTION_ROW = "com.hemang.mnemora.widget.ROW"
        const val ACTION_UNDO = "com.hemang.mnemora.widget.UNDO"
        const val EXTRA_KIND = "kind"
        const val EXTRA_TASK_ID = "task_id"
        const val KIND_TICK = "tick"
        const val KIND_OPEN = "open"

        private val executor = Executors.newSingleThreadExecutor()
    }
}
