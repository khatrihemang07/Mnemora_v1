package com.hemang.mnemora.widget.quickadd

import android.content.Context
import android.graphics.Canvas
import android.graphics.CornerPathEffect
import android.graphics.Paint
import android.graphics.Path
import android.view.KeyEvent
import android.widget.EditText

/**
 * The Quick add title field. It paints a soft rounded wash behind the words Rust recognised, as
 * the app's Capture box does, but only while the marks belong to exactly the text on screen, so
 * the wash never sits on the wrong letters.
 */
class CaptureEditText(context: Context) : EditText(context) {
    private var marks: List<TextSpan> = emptyList()
    private var marksFor: String = ""
    private val path = Path()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.FILL
        pathEffect = CornerPathEffect(4f * resources.displayMetrics.density)
    }

    /** Called when the caret or selection moves (the `#` suggestions follow the caret). */
    var onCaretMoved: (() -> Unit)? = null

    /**
     * Called for Back while the keyboard is up. Without it the keyboard takes the first Back to hide
     * itself, and the sheet would need two. Returns whether it handled the key.
     */
    var onBack: (() -> Boolean)? = null

    fun setHighlight(color: Int) {
        paint.color = color
    }

    /** Marks [spans] of [text]; they show only while the field still holds [text]. */
    fun setMarks(spans: List<TextSpan>, text: String) {
        marks = spans
        marksFor = text
        invalidate()
    }

    override fun onKeyPreIme(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP && !event.isCanceled) {
            if (onBack?.invoke() == true) return true
        }
        return super.onKeyPreIme(keyCode, event)
    }

    override fun onSelectionChanged(selStart: Int, selEnd: Int) {
        super.onSelectionChanged(selStart, selEnd)
        onCaretMoved?.invoke()
    }

    override fun onDraw(canvas: Canvas) {
        val l = layout
        if (l != null && marks.isNotEmpty() && text.toString() == marksFor) {
            canvas.save()
            canvas.translate(compoundPaddingLeft.toFloat(), extendedPaddingTop.toFloat())
            for (m in marks) {
                if (m.start < m.end && m.end <= text.length) {
                    l.getSelectionPath(m.start, m.end, path)
                    canvas.drawPath(path, paint)
                }
            }
            canvas.restore()
        }
        super.onDraw(canvas)
    }
}
