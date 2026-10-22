package com.hemang.mnemora.widget

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

/** Draws every Todo widget, off the main thread, and keeps the midnight redraw armed (a job, see [WidgetJobs]). */
object WidgetUpdater {
    private const val TAG = "TodoWidget"
    private const val ACTION_TODO_CHANGED = "com.hemang.mnemora.widget.TODO_CHANGED"

    private val executor = Executors.newSingleThreadExecutor()
    private val queued = AtomicBoolean(false)

    /** When this process last drew, or 0: lets a resize skip a redraw that would show the same. */
    @Volatile
    private var lastDrawAt = 0L

    /** True for the broadcasts that mean "redraw all widgets". */
    fun isRedrawAction(action: String?) = when (action) {
        ACTION_TODO_CHANGED,
        // The clock or zone moved, so Today and Overdue may have changed. (DATE_CHANGED is never
        // delivered to a manifest receiver; the midnight job covers the day change.)
        Intent.ACTION_TIME_CHANGED, Intent.ACTION_TIMEZONE_CHANGED -> true
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
        redrawAsync(context) { pending?.finish() }
    }

    /** Like [redrawAsync] with a PendingResult; [done] runs when the redraw (or the one it folded into) is under way or finished. */
    fun redrawAsync(context: Context, done: () -> Unit) {
        val app = context.applicationContext
        if (!queued.compareAndSet(false, true)) {
            done()
            return
        }
        executor.execute {
            queued.set(false)
            try {
                redraw(app)
            } catch (e: Throwable) {
                Log.w(TAG, "redraw failed", e)
            } finally {
                done()
            }
        }
    }

    /**
     * The launcher reports a new widget size. The responsive RemoteViews already pick the layout
     * for it, so this only redraws when the process has not drawn lately (see [ResizeRedraw]).
     */
    fun redrawOnResize(context: Context, pending: BroadcastReceiver.PendingResult?) {
        if (ResizeRedraw.due(lastDrawAt, System.currentTimeMillis())) redrawAsync(context, pending) else pending?.finish()
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
     * Makes sure the Undo bar comes down when its time is up, even if this process is frozen by
     * then (the vivo freezer): the tap's receiver stays open until the bar ends and then redraws
     * (see WidgetActionReceiver), and this job is the backstop if the process died. The draw itself
     * checks the clock, so all either has to do is cause a draw.
     */
    fun hideUndoAt(context: Context, until: Long) {
        WidgetJobs.armUndoExpiry(context, until)
    }

    @Synchronized
    private fun redraw(context: Context) {
        val manager = AppWidgetManager.getInstance(context)
        val ids = manager.getAppWidgetIds(ComponentName(context, TodoWidgetProvider::class.java))
        WidgetConfigStore.retainOnly(context, ids)
        // A bar that is over goes, with the Undo token it holds; with none left the backstop is not needed.
        if (!UndoBar.settle(context, System.currentTimeMillis())) WidgetJobs.cancelUndoExpiry(context)
        if (ids.isEmpty()) {
            WidgetJobs.cancelMidnight(context)
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
        lastDrawAt = System.currentTimeMillis()
        WidgetJobs.armMidnight(context, zone)
    }

    /** Called when the last widget is removed. */
    fun cancelMidnight(context: Context) {
        WidgetJobs.cancelMidnight(context)
        WidgetJobs.cancelUndoExpiry(context)
    }
}
