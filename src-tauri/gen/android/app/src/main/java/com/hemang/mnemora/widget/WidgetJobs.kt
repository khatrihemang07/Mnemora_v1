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

    /** Keeps the process running until the Undo bar ends, then redraws so the bar goes. */
    const val UNDO_ID = 4402

    /** A little after the bar's end, so the clock has certainly passed it. */
    const val UNDO_SLACK_MS = 150L

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

    /**
     * Starts the job that waits for the Undo bar to end. It has no delay on purpose: a job that is
     * running keeps the process out of the freezer, which a timer or an alarm cannot, and a job
     * with a delay would run up to half a minute late. It reads the bar's end itself (see
     * [WidgetJobService]), so a newer tick simply replaces it.
     */
    fun armUndoExpiry(context: Context) = schedule(context, UNDO_ID, 0L)

    fun cancelUndoExpiry(context: Context) = scheduler(context).cancel(UNDO_ID)
}

/**
 * Runs the jobs above. The midnight job redraws every Todo widget (which re-arms it); the Undo job
 * waits in this running service until the bar ends and then redraws, which hides the bar and clears
 * its Undo token. Not exported; only the system can bind it.
 */
class WidgetJobService : JobService() {
    @Volatile
    private var undoWaiter: Thread? = null

    override fun onStartJob(params: JobParameters): Boolean {
        val app = applicationContext
        if (params.jobId != WidgetJobs.UNDO_ID) {
            WidgetUpdater.redrawAsync(app) { jobFinished(params, false) }
            return true
        }
        undoWaiter = Thread {
            try {
                // The bar's end can move while waiting (a newer tick replaces this job, so this is rare).
                while (true) {
                    val until = UndoBar.storedUntil(app)
                    val wait = until - System.currentTimeMillis()
                    if (until == 0L || wait <= 0L || wait > UndoBar.DURATION_MS + UndoBar.DIALOG_GRACE_MS) break
                    Thread.sleep(wait + WidgetJobs.UNDO_SLACK_MS)
                }
                WidgetUpdater.redrawAsync(app) { jobFinished(params, false) }
            } catch (e: InterruptedException) {
                // Stopped by the system or replaced by a newer job: nothing to do.
            }
        }.also { it.start() }
        return true
    }

    /** Never retried: the next redraw hides a stale bar anyway, by its timestamp. */
    override fun onStopJob(params: JobParameters): Boolean {
        undoWaiter?.interrupt()
        return false
    }
}
