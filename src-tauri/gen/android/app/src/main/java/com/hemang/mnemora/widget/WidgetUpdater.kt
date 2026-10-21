package com.hemang.mnemora.widget

import android.app.AlarmManager
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import java.time.ZoneId
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** Draws every Todo widget, off the main thread, and keeps the midnight redraw armed. */
object WidgetUpdater {
    private const val TAG = "TodoWidget"
    private const val ACTION_TODO_CHANGED = "com.hemang.mnemora.widget.TODO_CHANGED"
    private const val ACTION_MIDNIGHT = "com.hemang.mnemora.widget.MIDNIGHT"

    private val executor = Executors.newSingleThreadExecutor()
    private val queued = AtomicBoolean(false)

    /** True for the broadcasts that mean "redraw all widgets". */
    fun isRedrawAction(action: String?) = when (action) {
        ACTION_TODO_CHANGED, ACTION_MIDNIGHT,
        // The day or clock moved, so Today and Overdue may have changed.
        Intent.ACTION_TIME_CHANGED, Intent.ACTION_TIMEZONE_CHANGED, Intent.ACTION_DATE_CHANGED -> true
        else -> false
    }

    /** Todo data changed in the app: ask the provider to redraw all widgets. Safe on any thread. */
    fun signalChanged(context: Context) {
        context.sendBroadcast(
            Intent(context, TodoWidgetProvider::class.java).setAction(ACTION_TODO_CHANGED),
        )
    }

    /**
     * Redraws all widgets soon. A request that arrives while another is still waiting to start is
     * folded into it, since that one will read the newest data anyway. [pending] is finished when done.
     */
    fun redrawAsync(context: Context, pending: BroadcastReceiver.PendingResult?) {
        val app = context.applicationContext
        if (!queued.compareAndSet(false, true)) {
            pending?.finish()
            return
        }
        executor.execute {
            queued.set(false)
            try {
                redraw(app)
            } catch (e: Throwable) {
                Log.w(TAG, "redraw failed", e)
            } finally {
                pending?.finish()
            }
        }
    }

    private fun redraw(context: Context) {
        val manager = AppWidgetManager.getInstance(context)
        val ids = manager.getAppWidgetIds(ComponentName(context, TodoWidgetProvider::class.java))
        if (ids.isEmpty()) {
            cancelMidnight(context)
            return
        }
        if (Build.VERSION.SDK_INT < 31) return
        val zone = ZoneId.systemDefault()
        val now = System.currentTimeMillis()
        // Widgets showing the same view share one read.
        val replies = HashMap<String, Reply>()
        for (id in ids) {
            val config = WidgetConfigStore.get(context, id)
            val reply = replies.getOrPut(config.view) {
                ReplyParser.parse(WidgetBridge.call(context, WidgetRequests.snapshot(config.view, now, zone)))
            }
            (reply as? Reply.Ok)?.let { WidgetConfigStore.rememberAppTheme(context, it.snapshot.appTheme) }
            manager.updateAppWidget(id, WidgetRenderer.render(context, reply, config, id))
        }
        armMidnight(context, zone)
    }

    private fun midnightIntent(context: Context): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            0,
            Intent(context, TodoWidgetProvider::class.java).setAction(ACTION_MIDNIGHT),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

    /** Inexact on purpose (a 10 minute window): no exact-alarm permission, and never an hour late. Re-armed on every update. */
    private fun armMidnight(context: Context, zone: ZoneId) {
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val at = Midnight.nextAfter(System.currentTimeMillis(), zone)
        alarms.setWindow(AlarmManager.RTC, at, Midnight.WINDOW_MS, midnightIntent(context))
    }

    /** Called when the last widget is removed. */
    fun cancelMidnight(context: Context) {
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        alarms.cancel(midnightIntent(context))
    }
}
