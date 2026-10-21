package com.hemang.mnemora.widget

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.res.ColorStateList
import android.view.View
import android.widget.RemoteViews
import androidx.annotation.RequiresApi
import com.hemang.mnemora.BuildConfig
import com.hemang.mnemora.MainActivity
import com.hemang.mnemora.R

/** Turns a bridge [Reply] into the RemoteViews the home screen shows. Android 12+ only (ADR 0015). */
@RequiresApi(31)
object WidgetRenderer {
    /** The card with skeleton rows, shown until the first real draw. */
    fun loading(context: Context): RemoteViews = RemoteViews(context.packageName, R.layout.widget_skeleton)

    fun render(context: Context, reply: Reply): RemoteViews {
        val palette = Palette.from(context)
        val views = RemoteViews(context.packageName, R.layout.widget_card)
        views.setColorStateList(R.id.root, "setBackgroundTintList", ColorStateList.valueOf(palette.background))
        val open = openApp(context)
        views.setOnClickPendingIntent(R.id.root, open)
        views.setViewVisibility(R.id.sandbox_tag, if (BuildConfig.SANDBOX) View.VISIBLE else View.GONE)
        when (reply) {
            is Reply.Ok -> ok(context, views, palette, reply.snapshot, open)
            Reply.NotSetUp -> message(
                views, palette,
                context.getString(R.string.widget_not_set_up_title),
                context.getString(R.string.widget_not_set_up_line),
            )
            Reply.NeedsUpdate -> message(
                views, palette,
                context.getString(R.string.widget_needs_update_title),
                context.getString(R.string.widget_needs_update_line),
            )
            Reply.Failed -> message(
                views, palette,
                context.getString(R.string.widget_failed_title),
                context.getString(R.string.widget_failed_line),
            )
        }
        return views
    }

    /** A status or error: the card without its header, one centred message that opens the app. */
    private fun message(views: RemoteViews, palette: Palette, title: String, line: String) {
        views.setViewVisibility(R.id.header, View.GONE)
        views.setViewVisibility(R.id.list, View.GONE)
        showMessage(views, palette, title, line)
    }

    private fun showMessage(views: RemoteViews, palette: Palette, title: String, line: String) {
        views.setViewVisibility(R.id.message, View.VISIBLE)
        views.setTextViewText(R.id.message_title, title)
        views.setTextColor(R.id.message_title, palette.ink)
        views.setTextViewText(R.id.message_line, line)
        views.setTextColor(R.id.message_line, palette.muted)
    }

    private fun ok(
        context: Context,
        views: RemoteViews,
        palette: Palette,
        snapshot: Snapshot,
        open: PendingIntent,
    ) {
        views.setViewVisibility(R.id.header, View.VISIBLE)
        views.setOnClickPendingIntent(R.id.header, open)
        views.setColorStateList(R.id.chip, "setBackgroundTintList", ColorStateList.valueOf(Palette.withAlpha(palette.todo, 0.16f)))
        views.setInt(R.id.chip_icon, "setColorFilter", palette.todo)
        views.setTextViewText(R.id.title, snapshot.title)
        views.setTextColor(R.id.title, palette.ink)
        val count = snapshot.count
        if (count == null) {
            views.setViewVisibility(R.id.count, View.GONE)
        } else {
            views.setViewVisibility(R.id.count, View.VISIBLE)
            views.setTextViewText(R.id.count, count.text)
            views.setTextColor(R.id.count, if (count.tone == Tone.OVERDUE) palette.danger else palette.faint)
        }
        val empty = snapshot.empty
        if (empty != null) {
            views.setViewVisibility(R.id.list, View.GONE)
            showMessage(views, palette, empty.title, empty.line)
            return
        }
        views.setViewVisibility(R.id.message, View.GONE)
        views.setViewVisibility(R.id.list, View.VISIBLE)
        views.setPendingIntentTemplate(R.id.list, open)
        views.setRemoteAdapter(R.id.list, items(context, palette, RowItems.flatten(snapshot.groups)))
    }

    private fun items(context: Context, palette: Palette, lines: List<Item>): RemoteViews.RemoteCollectionItems {
        val builder = RemoteViews.RemoteCollectionItems.Builder()
            .setHasStableIds(true)
            .setViewTypeCount(2)
        lines.forEachIndexed { index, line ->
            when (line) {
                is Item.Heading -> builder.addItem(-(index + 1L), heading(context, palette, line))
                is Item.Task -> builder.addItem(line.row.id.hashCode().toLong(), task(context, palette, line.row))
            }
        }
        return builder.build()
    }

    private fun heading(context: Context, palette: Palette, line: Item.Heading): RemoteViews {
        val views = RemoteViews(context.packageName, R.layout.widget_group)
        views.setTextViewText(R.id.group_title, line.text.uppercase())
        views.setTextColor(R.id.group_title, if (line.tone == Tone.OVERDUE) palette.danger else palette.faint)
        return views
    }

    private fun task(context: Context, palette: Palette, row: TaskRow): RemoteViews {
        val views = RemoteViews(context.packageName, R.layout.widget_row)
        views.setTextViewText(R.id.row_title, row.title)
        views.setTextColor(R.id.row_title, palette.ink)
        views.setInt(R.id.diamond_edge, "setColorFilter", palette.ringEdge(row.ring, row.overdue))
        val fill = palette.ringFill(row.ring)
        views.setViewVisibility(R.id.diamond_fill, if (fill == 0) View.INVISIBLE else View.VISIBLE)
        if (fill != 0) views.setInt(R.id.diamond_fill, "setColorFilter", fill)
        // An Overdue row shows the day it was due in red where the time would go.
        val right = row.day ?: row.time
        if (right == null) {
            views.setViewVisibility(R.id.row_time, View.GONE)
        } else {
            views.setViewVisibility(R.id.row_time, View.VISIBLE)
            views.setTextViewText(R.id.row_time, right)
            views.setTextColor(R.id.row_time, if (row.day != null) palette.danger else palette.faint)
        }
        if (row.list == null) {
            views.setViewVisibility(R.id.row_chip, View.GONE)
        } else {
            views.setViewVisibility(R.id.row_chip, View.VISIBLE)
            views.setTextViewText(R.id.row_chip, row.list)
            views.setTextColor(R.id.row_chip, palette.muted)
            views.setColorStateList(R.id.row_chip, "setBackgroundTintList", ColorStateList.valueOf(palette.surface2))
        }
        views.setOnClickFillInIntent(R.id.row_root, Intent())
        return views
    }

    /** Opens the app. The explicit component keeps Production and Sandbox apart. */
    private fun openApp(context: Context): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .setAction(Intent.ACTION_MAIN)
            .addCategory(Intent.CATEGORY_LAUNCHER)
        // Mutable so list rows can fill in their own details later; explicit, so safe.
        return PendingIntent.getActivity(
            context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
        )
    }
}
