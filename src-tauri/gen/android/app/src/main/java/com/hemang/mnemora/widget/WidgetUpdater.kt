package com.hemang.mnemora.widget

import android.app.AlarmManager
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.time.ZoneId
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** Draws every Todo widget, off the main thread, and keeps the midnight redraw armed. */
object WidgetUpdater {
    private const val TAG = "TodoWidget"
    private const val ACTION_TODO_CHANGED = "com.hemang.mnemora.widget.TODO_CHANGED"
    private const val ACTION_MIDNIGHT = "com.hemang.mnemora.widget.MIDNIGHT"
    private const val ACTION_UNDO_EXPIRED = "com.hemang.mnemora.widget.UNDO_EXPIRED"

    private val executor = Executors.newSingleThreadExecutor()
    private val queued = AtomicBoolean(false)

    /** True for the broadcasts that mean "redraw all widgets". */
    fun isRedrawAction(action: String?) = when (action) {
        ACTION_TODO_CHANGED, ACTION_MIDNIGHT, ACTION_UNDO_EXPIRED,
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

    /** Redraws all widgets now, on the calling thread (which must not be the main one). Never throws. */
    fun redrawNow(context: Context) {
        try {
            redraw(context.applicationContext)
        } catch (e: Throwable) {
            Log.w(TAG, "redraw failed", e)
        }
    }

    /**
     * Takes the Undo bar down when its time is up. The draw itself checks the clock, so this only has
     * to cause a draw: a timer while this process lives, and an inexact alarm in case it does not.
     */
    fun hideUndoAt(context: Context, until: Long) {
        val app = context.applicationContext
        Handler(Looper.getMainLooper()).postDelayed(
            { redrawAsync(app, null) },
            (until - System.currentTimeMillis()).coerceAtLeast(0) + 100,
        )
        val alarms = app.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val expired = PendingIntent.getBroadcast(
            app,
            1,
            Intent(app, TodoWidgetProvider::class.java).setAction(ACTION_UNDO_EXPIRED),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        alarms.setWindow(AlarmManager.RTC, until, 15_000L, expired)
    }

    @Synchronized
    private fun redraw(context: Context) {
        val manager = AppWidgetManager.getInstance(context)
        val ids = manager.getAppWidgetIds(ComponentName(context, TodoWidgetProvider::class.java))
        WidgetConfigStore.retainOnly(context, ids)
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
            manager.updateAppWidget(id, WidgetRenderer.responsive(context, reply, config, id))
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
