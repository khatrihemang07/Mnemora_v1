package com.hemang.mnemora.widget

import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.annotation.RequiresApi
import com.hemang.mnemora.R

/**
 * "N steps are still open": the question a tick asks when the Task's notes hold unticked Checklist
 * items, as a translucent dialog over the home screen, dressed in the tapped widget's theme. The
 * choices are the app's: Tick all and complete, Complete, Cancel (which changes nothing).
 */
@RequiresApi(31)
class StepsDialogActivity : Activity() {
    private var busy = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val taskId = intent.getStringExtra(EXTRA_TASK_ID)
        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        val count = intent.getIntExtra(EXTRA_COUNT, 0)
        if (taskId == null || count <= 0) {
            finish()
            return
        }
        val widgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
        val config = WidgetConfigStore.get(this, widgetId)
        val look = WidgetTheme.resolve(config.theme, WidgetConfigStore.appTheme(this), WidgetRenderer.phoneNight(this))
        val palette = Palette.from(this, look)
        setFinishOnTouchOutside(true)

        fun dp(v: Int) = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v.toFloat(), resources.displayMetrics).toInt()
        fun text(value: String, sp: Float, color: Int, bold: Boolean = false) = TextView(this).apply {
            this.text = value
            setTextSize(TypedValue.COMPLEX_UNIT_SP, sp)
            setTextColor(color)
            if (bold) typeface = Typeface.DEFAULT_BOLD
        }

        val card = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(palette.background)
                cornerRadius = dp(24).toFloat()
            }
            setPadding(dp(20), dp(20), dp(20), dp(12))
        }
        val heading = if (count == 1) getString(R.string.widget_steps_one) else getString(R.string.widget_steps_many, count)
        card.addView(text(heading, 18f, palette.ink, bold = true))
        card.addView(
            text(getString(R.string.widget_steps_line, title), 13f, palette.muted).apply { setPadding(0, dp(6), 0, dp(14)) },
        )
        fun action(label: Int, run: () -> Unit) = text(getString(label), 13.5f, palette.todo, bold = true).apply {
            setPadding(dp(10), dp(10), dp(10), dp(10))
            gravity = Gravity.END
            isClickable = true
            setOnClickListener { run() }
        }
        card.addView(action(R.string.widget_steps_tick_all) { answer(taskId, true) }, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        card.addView(action(R.string.widget_steps_complete) { answer(taskId, false) }, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        card.addView(action(R.string.widget_steps_cancel) { finish() }, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))

        val root = FrameLayout(this)
        root.addView(card, FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.CENTER).apply {
            marginStart = dp(20)
            marginEnd = dp(20)
        })
        // A tap outside the card is Cancel.
        root.setOnClickListener { finish() }
        card.isClickable = true
        setContentView(root)
    }

    /**
     * Hands the answer to the receiver and closes. The work must not run here: once this activity is
     * gone the process is cached and its threads are held back for seconds, while a receiver's
     * `goAsync` keeps it running (a tick from the dialog took 3.4 s that way).
     */
    private fun answer(taskId: String, tickSteps: Boolean) {
        if (busy) return
        busy = true
        sendBroadcast(WidgetActionReceiver.complete(this, taskId, tickSteps))
        finish()
    }

    companion object {
        private const val EXTRA_TASK_ID = "task_id"
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_COUNT = "count"

        fun intent(context: Context, question: TickReply.OpenSteps, widgetId: Int): Intent =
            Intent(context, StepsDialogActivity::class.java)
                .putExtra(EXTRA_TASK_ID, question.taskId)
                .putExtra(EXTRA_TITLE, question.title)
                .putExtra(EXTRA_COUNT, question.count)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
}
