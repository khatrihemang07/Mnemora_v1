package com.hemang.mnemora.widget.quickadd

import android.content.Context
import android.view.View
import android.view.ViewGroup

/** Lays its children out left to right and wraps to a new line when one does not fit (the chips). */
class FlowLayout(context: Context, private val hGapPx: Int, private val vGapPx: Int) : ViewGroup(context) {
    override fun onMeasure(widthSpec: Int, heightSpec: Int) {
        val maxWidth = MeasureSpec.getSize(widthSpec) - paddingLeft - paddingRight
        var x = 0
        var y = 0
        var lineHeight = 0
        var widest = 0
        for (i in 0 until childCount) {
            val child = getChildAt(i)
            if (child.visibility == View.GONE) continue
            child.measure(
                MeasureSpec.makeMeasureSpec(maxWidth, MeasureSpec.AT_MOST),
                MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED),
            )
            val w = child.measuredWidth
            val h = child.measuredHeight
            if (x > 0 && x + w > maxWidth) {
                y += lineHeight + vGapPx
                x = 0
                lineHeight = 0
            }
            x += w + hGapPx
            lineHeight = maxOf(lineHeight, h)
            widest = maxOf(widest, x - hGapPx)
        }
        val height = y + lineHeight + paddingTop + paddingBottom
        setMeasuredDimension(
            resolveSize(widest + paddingLeft + paddingRight, widthSpec),
            resolveSize(height, heightSpec),
        )
    }

    override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
        val maxWidth = r - l - paddingLeft - paddingRight
        var x = 0
        var y = 0
        var lineHeight = 0
        for (i in 0 until childCount) {
            val child = getChildAt(i)
            if (child.visibility == View.GONE) continue
            val w = child.measuredWidth
            val h = child.measuredHeight
            if (x > 0 && x + w > maxWidth) {
                y += lineHeight + vGapPx
                x = 0
                lineHeight = 0
            }
            child.layout(paddingLeft + x, paddingTop + y, paddingLeft + x + w, paddingTop + y + h)
            x += w + hGapPx
            lineHeight = maxOf(lineHeight, h)
        }
    }
}
