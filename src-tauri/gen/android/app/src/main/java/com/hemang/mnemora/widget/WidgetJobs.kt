package com.hemang.mnemora.widget

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.util.Log
import java.time.ZoneId

/**
 * The Todo widget's timed redraws, as framework jobs rather than alarms. vivo's freezer freezes
 * this app seconds after it finishes work and then deletes every alarm it owns (`dumpsys alarm`
 * lists them removed, Reason=frozen), and DATE_CHANGED is never delivered to a manifest receiver.
 * A job belongs to the system, so it survives both and starts the process when it runs. It needs
 * no permission. It is not exact: it runs at the first device wake after its time, and within about
 * half a minute of it while the screen is on.
 */
object WidgetJobs {
    private const val TAG = "TodoWidget"

    /** The redraw at the next local midnight. Stable, so arming it again replaces the pending one. */
    const val MIDNIGHT_ID = 4401

    /** The backstop that takes the Undo bar down, should the process not do it itself. */
    const val UNDO_ID = 4402

    /** A little after the bar's end, so the clock has certainly passed it. */
    private const val UNDO_SLACK_MS = 100L

    private fun scheduler(context: Context): JobScheduler =
        context.applicationContext.getSystemService(JobScheduler::class.java)

    private fun schedule(context: Context, id: Int, delayMs: Long) {
        val info = JobInfo.Builder(id, ComponentName(context.applicationContext, WidgetJobService::class.java))
            // No deadline (it would add nothing) and no constraints.
            .setMinimumLatency(delayMs.coerceAtLeast(0L))
            .build()
        if (scheduler(context).schedule(info) != JobScheduler.RESULT_SUCCESS) Log.w(TAG, "job $id not scheduled")
    }

    /** Redraw soon after the next local midnight. Called on every redraw, so a clock or zone change corrects it. */
    fun armMidnight(context: Context, zone: ZoneId) {
        schedule(context, MIDNIGHT_ID, Midnight.delayFrom(System.currentTimeMillis(), zone))
    }

    fun cancelMidnight(context: Context) = scheduler(context).cancel(MIDNIGHT_ID)

    /** Redraw soon after the Undo bar ending at [until], which hides it. */
    fun armUndoExpiry(context: Context, until: Long) {
        schedule(context, UNDO_ID, until - System.currentTimeMillis() + UNDO_SLACK_MS)
    }

    fun cancelUndoExpiry(context: Context) = scheduler(context).cancel(UNDO_ID)
}

/**
 * Runs the jobs above: a redraw of every Todo widget, which re-arms the midnight job and hides an
 * Undo bar that is over. Not exported; only the system can bind it.
 */
class WidgetJobService : JobService() {
    override fun onStartJob(params: JobParameters): Boolean {
        WidgetUpdater.redrawAsync(applicationContext) { jobFinished(params, false) }
        return true
    }

    /** Never retried: the next redraw arms what is needed again. */
    override fun onStopJob(params: JobParameters): Boolean = false
}
