package com.hemang.mnemora.widget

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.appwidget.AppWidgetManager
import android.content.res.ColorStateList
import android.content.res.Configuration
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

    /** Whether the phone is in night mode right now. */
    fun phoneNight(context: Context): Boolean =
        context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK ==
            Configuration.UI_MODE_NIGHT_YES

    /**
     * Draws [reply] for the widget [appWidgetId] as its [config] says. The same function draws the
     * setup screen's preview, so what you pick is what you get.
     */
    fun render(
        context: Context,
        reply: Reply,
        config: WidgetConfig,
        appWidgetId: Int = AppWidgetManager.INVALID_APPWIDGET_ID,
        previewLines: Int? = null,
    ): RemoteViews {
        // The app's theme is known from a reply; failures use the last one seen.
        val appTheme = (reply as? Reply.Ok)?.snapshot?.appTheme ?: WidgetConfigStore.appTheme(context)
        val look = WidgetTheme.resolve(config.theme, appTheme, phoneNight(context))
        val palette = Palette.from(context, look, config.background)
        val edge = TextEdge.of(config.background, look)
        val views = RemoteViews(context.packageName, edge.card)
        // With no background a faint wash still sits behind the text, so it reads on any wallpaper.
        val surface = if (config.background == Background.NONE) Palette.scrim(look) else Palette.withAlpha(palette.background, config.background.alpha)
        views.setColorStateList(R.id.root, "setBackgroundTintList", ColorStateList.valueOf(surface))
        val open = openApp(context)
        views.setOnClickPendingIntent(R.id.root, open)
        views.setViewVisibility(R.id.sandbox_tag, if (BuildConfig.SANDBOX) View.VISIBLE else View.GONE)
        undoBar(context, views, palette, config.background, appWidgetId)
        when (reply) {
            is Reply.Ok -> ok(context, views, palette, edge, reply.snapshot, open, previewLines, appWidgetId)
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
            Reply.ListMissing -> {
                message(
                    views, palette,
                    context.getString(R.string.widget_list_deleted_title),
                    context.getString(R.string.widget_list_deleted_line),
                )
                // Tapping it opens setup, to point the widget somewhere else.
                if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID) {
                    views.setOnClickPendingIntent(R.id.root, setupIntent(context, appWidgetId))
                }
                // No header here, so the gear sits in the corner.
                views.setViewVisibility(R.id.gear_corner, View.VISIBLE)
                views.setColorStateList(R.id.gear_corner, "setImageTintList", ColorStateList.valueOf(palette.faint))
                gearClick(context, views, R.id.gear_corner, appWidgetId)
            }
            Reply.Failed -> message(
                views, palette,
                context.getString(R.string.widget_failed_title),
                context.getString(R.string.widget_failed_line),
            )
        }
        return views
    }

    /** The Undo bar after a tick, in the widget's own theme; hidden once its time is up. */
    private fun undoBar(context: Context, views: RemoteViews, palette: Palette, background: Background, appWidgetId: Int) {
        // The setup screen's preview is not a real widget: no bar there.
        val state = if (appWidgetId == AppWidgetManager.INVALID_APPWIDGET_ID) null else UndoBar.get(context, System.currentTimeMillis())
        if (state == null) {
            views.setViewVisibility(R.id.undo_bar, View.GONE)
            return
        }
        // Over a bare wallpaper the bar is its own light card, whatever the theme.
        val bare = background == Background.NONE
        val fill = if (bare) Palette.withAlpha(0xFFFFFFFF.toInt(), 0.92f) else palette.surface2
        val ink = if (bare) 0xFF1D1712.toInt() else palette.ink
        views.setViewVisibility(R.id.undo_bar, View.VISIBLE)
        views.setColorStateList(R.id.undo_bar, "setBackgroundTintList", ColorStateList.valueOf(fill))
        views.setTextViewText(R.id.undo_text, state.message)
        views.setTextColor(R.id.undo_text, ink)
        views.setTextColor(R.id.undo_action, palette.todo)
        views.setOnClickPendingIntent(R.id.undo_action, actionIntent(context, WidgetActionReceiver.ACTION_UNDO, 2, FLAG_IMMUTABLE_ONLY))
    }

    /** A broadcast to the receiver that ticks and undoes. Explicit, so it is safe even when mutable. */
    private fun actionIntent(context: Context, action: String, requestCode: Int, flags: Int): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            requestCode,
            Intent(context, WidgetActionReceiver::class.java).setAction(action),
            PendingIntent.FLAG_UPDATE_CURRENT or flags,
        )

    private const val FLAG_IMMUTABLE_ONLY = PendingIntent.FLAG_IMMUTABLE

    /** Makes a gear open this widget's setup; without a real widget (the setup preview) it does nothing. */
    private fun gearClick(context: Context, views: RemoteViews, id: Int, appWidgetId: Int) {
        if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID) {
            views.setOnClickPendingIntent(id, setupIntent(context, appWidgetId))
        }
    }

    /** Opens the setup screen for one widget (the gear, and a deleted List's card). */
    fun setupIntent(context: Context, appWidgetId: Int): PendingIntent {
        val intent = Intent(context, WidgetSetupActivity::class.java)
            .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
        // The widget id is the request code: each widget gets its own PendingIntent.
        return PendingIntent.getActivity(
            context, appWidgetId, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
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
        edge: TextEdge,
        snapshot: Snapshot,
        open: PendingIntent,
        previewLines: Int?,
        appWidgetId: Int,
    ) {
        views.setViewVisibility(R.id.header, View.VISIBLE)
        views.setColorStateList(R.id.gear, "setImageTintList", ColorStateList.valueOf(palette.faint))
        gearClick(context, views, R.id.gear, appWidgetId)
        views.setOnClickPendingIntent(R.id.header, open)
        views.setColorStateList(R.id.chip, "setBackgroundTintList", ColorStateList.valueOf(Palette.withAlpha(palette.todo, 0.16f)))
        views.setColorStateList(R.id.chip_icon, "setImageTintList", ColorStateList.valueOf(palette.todo))
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
        // New contents start at the top, not where the last draw was scrolled to.
        views.setScrollPosition(R.id.list, 0)
        // Row taps are broadcasts to the receiver; each row's fill-in says which part was tapped.
        views.setPendingIntentTemplate(R.id.list, actionIntent(context, WidgetActionReceiver.ACTION_ROW, 3, PendingIntent.FLAG_MUTABLE))
        views.setRemoteAdapter(R.id.list, items(context, palette, edge, appWidgetId, previewLines?.let { RowItems.preview(snapshot.groups, it) } ?: RowItems.flatten(snapshot.groups)))
    }

    private fun items(
        context: Context,
        palette: Palette,
        edge: TextEdge,
        appWidgetId: Int,
        lines: List<Item>,
    ): RemoteViews.RemoteCollectionItems {
        val builder = RemoteViews.RemoteCollectionItems.Builder()
            .setHasStableIds(true)
            .setViewTypeCount(2)
        lines.forEachIndexed { index, line ->
            when (line) {
                is Item.Heading -> builder.addItem(-(index + 1L), heading(context, palette, edge, line))
                is Item.Task -> builder.addItem(line.row.id.hashCode().toLong(), task(context, palette, edge, appWidgetId, line.row))
            }
        }
        return builder.build()
    }

    private fun heading(context: Context, palette: Palette, edge: TextEdge, line: Item.Heading): RemoteViews {
        val views = RemoteViews(context.packageName, edge.group)
        views.setTextViewText(R.id.group_title, line.text.uppercase())
        views.setTextColor(R.id.group_title, if (line.tone == Tone.OVERDUE) palette.danger else palette.faint)
        return views
    }

    private fun task(context: Context, palette: Palette, edge: TextEdge, appWidgetId: Int, row: TaskRow): RemoteViews {
        val views = RemoteViews(context.packageName, edge.row)
        views.setTextViewText(R.id.row_title, row.title)
        views.setTextColor(R.id.row_title, palette.ink)
        // A tint (not setColorFilter) keeps a translucent fill translucent over the drawable's white.
        views.setColorStateList(
            R.id.diamond_edge, "setImageTintList",
            ColorStateList.valueOf(palette.ringEdge(row.ring, row.overdue)),
        )
        val fill = palette.ringFill(row.ring)
        views.setViewVisibility(R.id.diamond_fill, if (fill == 0) View.INVISIBLE else View.VISIBLE)
        if (fill != 0) views.setColorStateList(R.id.diamond_fill, "setImageTintList", ColorStateList.valueOf(fill))
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
        views.setOnClickFillInIntent(
            R.id.row_root,
            Intent().putExtra(WidgetActionReceiver.EXTRA_KIND, WidgetActionReceiver.KIND_OPEN),
        )
        // The diamond ticks.
        views.setOnClickFillInIntent(
            R.id.diamond,
            Intent()
                .putExtra(WidgetActionReceiver.EXTRA_KIND, WidgetActionReceiver.KIND_TICK)
                .putExtra(WidgetActionReceiver.EXTRA_TASK_ID, row.id)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId),
        )
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

/**
 * How the widget's text is edged. On a Solid or See-through card there is no edge; with no
 * background the text sits on the wallpaper, so dark themes get a soft dark shadow and light
 * themes a soft light glow (RemoteViews cannot set either at draw time, hence a layout set each).
 */
enum class TextEdge(val card: Int, val row: Int, val group: Int) {
    NONE(R.layout.widget_card, R.layout.widget_row, R.layout.widget_group),
    SHADOW(R.layout.widget_card_shadow, R.layout.widget_row_shadow, R.layout.widget_group_shadow),
    GLOW(R.layout.widget_card_glow, R.layout.widget_row_glow, R.layout.widget_group_glow),
    ;

    companion object {
        fun of(background: Background, look: Look): TextEdge = when {
            background != Background.NONE -> NONE
            WidgetTheme.isDark(look) -> SHADOW
            else -> GLOW
        }
    }
}
