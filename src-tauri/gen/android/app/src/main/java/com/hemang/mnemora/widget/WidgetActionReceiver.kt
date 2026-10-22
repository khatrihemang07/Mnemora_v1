package com.hemang.mnemora.widget

import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import com.hemang.mnemora.MainActivity
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

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
            var holding = false
            try {
                if (action == ACTION_UNDO) WidgetActions.undo(app) else row(app, intent)
                holding = holdForUndoBar(app, pending)
            } catch (e: Throwable) {
                Log.w(TAG, "widget action failed", e)
            } finally {
                if (!holding) pending.finish()
            }
        }
    }

    /**
     * After a tick the Undo bar must come down about four seconds later. The vivo freezer freezes
     * this process seconds after it goes idle, which stops a timer and deletes alarms, but not a
     * process that is busy in a receiver. So the receiver stays open until the bar ends (well inside
     * the broadcast limit), then redraws; WidgetJobs holds a job as the backstop if the process dies.
     * Returns whether [pending] now belongs to that wait.
     */
    private fun holdForUndoBar(context: Context, pending: PendingResult): Boolean {
        val until = UndoBar.get(context, System.currentTimeMillis())?.until ?: return false
        val wait = (until - System.currentTimeMillis()).coerceAtLeast(0L) + EXPIRY_SLACK_MS
        expiry.schedule({ WidgetUpdater.redrawAsync(context) { pending.finish() } }, wait, TimeUnit.MILLISECONDS)
        return true
    }

    private fun row(context: Context, intent: Intent) {
        when (intent.getStringExtra(EXTRA_KIND)) {
            // The steps dialog's answer: tickSteps says which of its two completing choices was taken.
            KIND_COMPLETE -> {
                val id = intent.getStringExtra(EXTRA_TASK_ID) ?: return
                WidgetActions.complete(context, id, intent.getBooleanExtra(EXTRA_TICK_STEPS, false))
            }
            KIND_TICK -> {
                val id = intent.getStringExtra(EXTRA_TASK_ID) ?: return
                val reply = WidgetActions.complete(context, id, null)
                if (reply is TickReply.OpenSteps) {
                    val widgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
                    context.startActivity(StepsDialogActivity.intent(context, reply, widgetId))
                }
            }
            // Any other part of a row opens that Task over the widget's view.
            else -> {
                val link = intent.getStringExtra(EXTRA_LINK)
                val open = if (link != null && WidgetLinks.isLink(link)) {
                    WidgetLinks.intent(context, link)
                } else {
                    Intent(context, MainActivity::class.java)
                        .setAction(Intent.ACTION_MAIN)
                        .addCategory(Intent.CATEGORY_LAUNCHER)
                }
                context.startActivity(open.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            }
        }
    }

    companion object {
        private const val TAG = "TodoWidget"
        const val ACTION_ROW = "com.hemang.mnemora.widget.ROW"
        const val ACTION_UNDO = "com.hemang.mnemora.widget.UNDO"
        const val EXTRA_KIND = "kind"
        const val EXTRA_TASK_ID = "task_id"
        const val EXTRA_LINK = "link"
        const val KIND_TICK = "tick"
        const val KIND_OPEN = "open"
        const val KIND_COMPLETE = "complete"
        const val EXTRA_TICK_STEPS = "tick_steps"

        /** The broadcast that completes a Task with the answer to its open steps. Explicit, so only this app's receiver gets it. */
        fun complete(context: Context, taskId: String, tickSteps: Boolean): Intent =
            Intent(context, WidgetActionReceiver::class.java)
                .setAction(ACTION_ROW)
                .putExtra(EXTRA_KIND, KIND_COMPLETE)
                .putExtra(EXTRA_TASK_ID, taskId)
                .putExtra(EXTRA_TICK_STEPS, tickSteps)

        private val executor = Executors.newSingleThreadExecutor()

        /** Ends the wait of a receiver that holds the process for the Undo bar. */
        private val expiry = Executors.newSingleThreadScheduledExecutor()

        private const val EXPIRY_SLACK_MS = 150L
    }
}
