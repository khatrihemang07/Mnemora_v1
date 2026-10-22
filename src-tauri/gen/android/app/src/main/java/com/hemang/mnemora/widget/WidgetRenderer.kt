package com.hemang.mnemora.widget

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.appwidget.AppWidgetManager
import android.content.res.Configuration
import android.util.SizeF
import android.view.View
import android.widget.RemoteViews
import androidx.annotation.RequiresApi
import com.hemang.mnemora.BuildConfig
import com.hemang.mnemora.MainActivity
import com.hemang.mnemora.R
import com.hemang.mnemora.widget.quickadd.QuickAddActivity

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
     * What the home screen gets for a real widget: the card at 3x2 and up, an "Add a task..." pill
     * from 100 dp wide and a plain + below that. The system picks one by the widget's size (see
     * [SizeMode]), so a resize needs no redraw.
     */
    fun responsive(context: Context, reply: Reply, config: WidgetConfig, appWidgetId: Int): RemoteViews {
        val sizes = LinkedHashMap<SizeF, RemoteViews>()
        for (b in SizeMode.BREAKPOINTS) {
            sizes[SizeF(b.widthDp, b.heightDp)] = when (b.mode) {
                SizeMode.LIST -> render(context, reply, config, appWidgetId)
                SizeMode.PILL, SizeMode.PLUS -> addButton(context, reply, config, appWidgetId, b.mode)
            }
        }
        return RemoteViews(sizes)
    }

    /** The colours a widget wears: its Theme setting, with "Same as app" read from the last reply. */
    private fun colorsOf(context: Context, reply: Reply, config: WidgetConfig): WidgetColors {
        // The app's theme is known from a reply; failures use the last one seen.
        val appTheme = (reply as? Reply.Ok)?.snapshot?.appTheme ?: WidgetConfigStore.appTheme(context)
        return WidgetColors.of(context, appTheme, config, phoneNight(context))
    }

    /** The card's fill: the theme's surface, a faint wash when there is no background. */
    private fun surfaceOf(palette: Palette, look: Look, background: Background): Int =
        if (background == Background.NONE) Palette.scrim(look) else Palette.withAlpha(palette.background, background.alpha)

    /** The pill or the plus: one tap opens the Quick add sheet for the widget's view. */
    private fun addButton(context: Context, reply: Reply, config: WidgetConfig, appWidgetId: Int, mode: SizeMode): RemoteViews {
        val colors = colorsOf(context, reply, config)
        val pill = mode == SizeMode.PILL
        val views = RemoteViews(context.packageName, if (pill) R.layout.widget_add_pill else R.layout.widget_add_plus)
        colors.tint(views, R.id.root, "setBackgroundTintList") { surfaceOf(palette, look, config.background) }
        colors.tint(views, R.id.add_icon, "setImageTintList") { palette.todo }
        if (pill) {
            colors.tint(views, R.id.add_field, "setBackgroundTintList") {
                if (config.background == Background.NONE) Palette.scrim(look) else palette.surface2
            }
            colors.text(views, R.id.add_label) { palette.muted }
        }
        views.setViewVisibility(R.id.sandbox_tag, if (BuildConfig.SANDBOX) View.VISIBLE else View.GONE)
        views.setContentDescription(R.id.root, context.getString(R.string.widget_add_task_description))
        if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID) {
            views.setOnClickPendingIntent(R.id.root, QuickAddActivity.pending(context, config.view, appWidgetId))
        }
        return views
    }

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
        val colors = colorsOf(context, reply, config)
        // The text edge is a layout choice, so it follows the look as drawn; a flip of night mode
        // changes it only at the next draw.
        val edge = TextEdge.of(config.background, colors.current.look)
        val views = RemoteViews(context.packageName, edge.card)
        // With no background a faint wash still sits behind the text, so it reads on any wallpaper.
        colors.tint(views, R.id.root, "setBackgroundTintList") { surfaceOf(palette, look, config.background) }
        views.setOnClickPendingIntent(R.id.root, openApp(context))
        views.setViewVisibility(R.id.sandbox_tag, if (BuildConfig.SANDBOX) View.VISIBLE else View.GONE)
        undoBar(context, views, colors, config.background, appWidgetId)
        when (reply) {
            is Reply.Ok -> ok(context, views, colors, edge, reply.snapshot, previewLines, appWidgetId)
            Reply.NotSetUp -> message(
                views, colors,
                context.getString(R.string.widget_not_set_up_title),
                context.getString(R.string.widget_not_set_up_line),
            )
            Reply.NeedsUpdate -> message(
                views, colors,
                context.getString(R.string.widget_needs_update_title),
                context.getString(R.string.widget_needs_update_line),
            )
            Reply.ListMissing -> {
                message(
                    views, colors,
                    context.getString(R.string.widget_list_deleted_title),
                    context.getString(R.string.widget_list_deleted_line),
                )
                // Tapping it opens setup, to point the widget somewhere else.
                if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID) {
                    views.setOnClickPendingIntent(R.id.root, setupIntent(context, appWidgetId))
                }
                // No header here, so the gear sits in the corner.
                views.setViewVisibility(R.id.gear_corner, View.VISIBLE)
                colors.tint(views, R.id.gear_corner, "setImageTintList") { palette.faint }
                gearClick(context, views, R.id.gear_corner, appWidgetId)
            }
            Reply.Failed -> message(
                views, colors,
                context.getString(R.string.widget_failed_title),
                context.getString(R.string.widget_failed_line),
            )
        }
        return views
    }

    /** The Undo bar after a tick, in the widget's own theme; hidden once its time is up. */
    private fun undoBar(context: Context, views: RemoteViews, colors: WidgetColors, background: Background, appWidgetId: Int) {
        // The setup screen's preview is not a real widget: no bar there.
        val state = if (appWidgetId == AppWidgetManager.INVALID_APPWIDGET_ID) null else UndoBar.get(context, System.currentTimeMillis())
        if (state == null) {
            views.setViewVisibility(R.id.undo_bar, View.GONE)
            return
        }
        // Over a bare wallpaper the bar is its own light card, whatever the theme.
        val bare = background == Background.NONE
        views.setViewVisibility(R.id.undo_bar, View.VISIBLE)
        // On a dark card surface-2 is barely lighter than the card, so the bar lifts with a wash of the ink.
        colors.tint(views, R.id.undo_bar, "setBackgroundTintList") {
            when {
                bare -> Palette.withAlpha(0xFFFFFFFF.toInt(), 0.92f)
                WidgetTheme.isDark(look) -> Palette.blend(palette.ink, palette.background, 0.16f)
                else -> palette.surface2
            }
        }
        views.setTextViewText(R.id.undo_text, state.lead)
        colors.text(views, R.id.undo_text) { if (bare) 0xFF1D1712.toInt() else palette.ink }
        // The title gives way; "· next Thu 29 Oct" stays whole.
        views.setViewVisibility(R.id.undo_suffix, if (state.suffix == null) View.GONE else View.VISIBLE)
        views.setTextViewText(R.id.undo_suffix, state.suffix.orEmpty())
        colors.text(views, R.id.undo_suffix) { if (bare) 0xFF1D1712.toInt() else palette.ink }
        colors.text(views, R.id.undo_action) { palette.todo }
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
    private fun message(views: RemoteViews, colors: WidgetColors, title: String, line: String) {
        views.setViewVisibility(R.id.header, View.GONE)
        views.setViewVisibility(R.id.list, View.GONE)
        showMessage(views, colors, title, line)
    }

    private fun showMessage(views: RemoteViews, colors: WidgetColors, title: String, line: String) {
        views.setViewVisibility(R.id.message, View.VISIBLE)
        views.setTextViewText(R.id.message_title, title)
        colors.text(views, R.id.message_title) { palette.ink }
        views.setTextViewText(R.id.message_line, line)
        colors.text(views, R.id.message_line) { palette.muted }
    }

    private fun ok(
        context: Context,
        views: RemoteViews,
        colors: WidgetColors,
        edge: TextEdge,
        snapshot: Snapshot,
        previewLines: Int?,
        appWidgetId: Int,
    ) {
        views.setViewVisibility(R.id.header, View.VISIBLE)
        colors.tint(views, R.id.gear, "setImageTintList") { palette.faint }
        gearClick(context, views, R.id.gear, appWidgetId)
        // The teal + sits right beside the gear and opens Quick add for this view.
        colors.tint(views, R.id.add, "setImageTintList") { palette.todo }
        if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID) {
            views.setOnClickPendingIntent(R.id.add, QuickAddActivity.pending(context, snapshot.view, appWidgetId))
        }
        views.setOnClickPendingIntent(R.id.header, linkIntent(context, WidgetLinks.build(snapshot.view), appWidgetId))
        colors.tint(views, R.id.chip, "setBackgroundTintList") { Palette.withAlpha(palette.todo, 0.16f) }
        colors.tint(views, R.id.chip_icon, "setImageTintList") { palette.todo }
        views.setTextViewText(R.id.title, snapshot.title)
        colors.text(views, R.id.title) { palette.ink }
        val count = snapshot.count
        if (count == null) {
            views.setViewVisibility(R.id.count, View.GONE)
        } else {
            views.setViewVisibility(R.id.count, View.VISIBLE)
            views.setTextViewText(R.id.count, count.text)
            colors.text(views, R.id.count) { if (count.tone == Tone.OVERDUE) palette.danger else palette.faint }
        }
        val empty = snapshot.empty
        if (empty != null) {
            views.setViewVisibility(R.id.list, View.GONE)
            showMessage(views, colors, empty.title, empty.line)
            return
        }
        views.setViewVisibility(R.id.message, View.GONE)
        views.setViewVisibility(R.id.list, View.VISIBLE)
        // New contents start at the top, not where the last draw was scrolled to.
        views.setScrollPosition(R.id.list, 0)
        // Row taps are broadcasts to the receiver; each row's fill-in says which part was tapped.
        views.setPendingIntentTemplate(R.id.list, actionIntent(context, WidgetActionReceiver.ACTION_ROW, 3, PendingIntent.FLAG_MUTABLE))
        views.setRemoteAdapter(R.id.list, items(context, colors, edge, appWidgetId, snapshot.view, previewLines?.let { RowItems.preview(snapshot.groups, it) } ?: RowItems.flatten(snapshot.groups)))
    }

    private fun items(
        context: Context,
        colors: WidgetColors,
        edge: TextEdge,
        appWidgetId: Int,
        view: String,
        lines: List<Item>,
    ): RemoteViews.RemoteCollectionItems {
        val builder = RemoteViews.RemoteCollectionItems.Builder()
            .setHasStableIds(true)
            .setViewTypeCount(2)
        lines.forEachIndexed { index, line ->
            when (line) {
                is Item.Heading -> builder.addItem(-(index + 1L), heading(context, colors, edge, line))
                is Item.Task -> builder.addItem(line.row.id.hashCode().toLong(), task(context, colors, edge, appWidgetId, view, line.row))
                is Item.More -> builder.addItem(-(index + 1L), more(context, colors, edge, view, line))
            }
        }
        return builder.build()
    }

    private fun heading(context: Context, colors: WidgetColors, edge: TextEdge, line: Item.Heading): RemoteViews {
        val views = RemoteViews(context.packageName, edge.group)
        views.setTextViewText(R.id.group_title, line.text.uppercase())
        colors.text(views, R.id.group_title) { if (line.tone == Tone.OVERDUE) palette.danger else palette.faint }
        return views
    }

    /** "+N more · Open Mnemora": the end of a list that was cut; a tap opens the view in the app. */
    private fun more(context: Context, colors: WidgetColors, edge: TextEdge, view: String, line: Item.More): RemoteViews {
        val views = RemoteViews(context.packageName, edge.group)
        views.setTextViewText(R.id.group_title, context.getString(R.string.widget_more_open, line.hidden))
        colors.text(views, R.id.group_title) { palette.todo }
        views.setOnClickFillInIntent(
            R.id.group_title,
            Intent()
                .putExtra(WidgetActionReceiver.EXTRA_KIND, WidgetActionReceiver.KIND_OPEN)
                .putExtra(WidgetActionReceiver.EXTRA_LINK, WidgetLinks.build(view)),
        )
        return views
    }

    private fun task(context: Context, colors: WidgetColors, edge: TextEdge, appWidgetId: Int, view: String, row: TaskRow): RemoteViews {
        val views = RemoteViews(context.packageName, edge.row)
        views.setTextViewText(R.id.row_title, row.title)
        colors.text(views, R.id.row_title) { palette.ink }
        // A tint (not setColorFilter) keeps a translucent fill translucent over the drawable's white.
        colors.tint(views, R.id.diamond_edge, "setImageTintList") { palette.ringEdge(row.ring, row.overdue) }
        // Whether a ring has a fill is the same in every look.
        val filled = colors.current.palette.ringFill(row.ring) != 0
        views.setViewVisibility(R.id.diamond_fill, if (filled) View.VISIBLE else View.INVISIBLE)
        if (filled) colors.tint(views, R.id.diamond_fill, "setImageTintList") { palette.ringFill(row.ring) }
        // An Overdue row shows the day it was due in red where the time would go.
        val right = row.day ?: row.time
        if (right == null) {
            views.setViewVisibility(R.id.row_time, View.GONE)
        } else {
            views.setViewVisibility(R.id.row_time, View.VISIBLE)
            views.setTextViewText(R.id.row_time, right)
            colors.text(views, R.id.row_time) { if (row.day != null) palette.danger else palette.faint }
        }
        if (row.list == null) {
            views.setViewVisibility(R.id.row_chip, View.GONE)
        } else {
            views.setViewVisibility(R.id.row_chip, View.VISIBLE)
            views.setTextViewText(R.id.row_chip, row.list)
            colors.text(views, R.id.row_chip) { palette.muted }
            colors.tint(views, R.id.row_chip, "setBackgroundTintList") { palette.surface2 }
        }
        views.setOnClickFillInIntent(
            R.id.row_root,
            Intent()
                .putExtra(WidgetActionReceiver.EXTRA_KIND, WidgetActionReceiver.KIND_OPEN)
                .putExtra(WidgetActionReceiver.EXTRA_LINK, WidgetLinks.build(view, row.id)),
        )
        // The diamond ticks; TalkBack says what it ticks.
        views.setContentDescription(R.id.diamond, context.getString(R.string.widget_complete_task, row.title))
        views.setOnClickFillInIntent(
            R.id.diamond,
            Intent()
                .putExtra(WidgetActionReceiver.EXTRA_KIND, WidgetActionReceiver.KIND_TICK)
                .putExtra(WidgetActionReceiver.EXTRA_TASK_ID, row.id)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId),
        )
        return views
    }

    /** Opens the app at [url] (a Todo link). Explicit and immutable: nothing can redirect it. */
    private fun linkIntent(context: Context, url: String, appWidgetId: Int): PendingIntent =
        PendingIntent.getActivity(
            context, appWidgetId, WidgetLinks.intent(context, url),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

    /** Opens the app. The explicit component keeps Production and Sandbox apart. */
    private fun openApp(context: Context): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .setAction(Intent.ACTION_MAIN)
            .addCategory(Intent.CATEGORY_LAUNCHER)
        // Immutable: rows use their own mutable template (ACTION_ROW), not this one.
        return PendingIntent.getActivity(
            context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
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
