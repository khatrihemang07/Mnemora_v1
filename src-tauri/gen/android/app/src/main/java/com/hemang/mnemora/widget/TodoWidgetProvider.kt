package com.hemang.mnemora.widget

import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.Context
import android.content.Intent

/** The Todo widget. Offered on Android 12+ only (ADR 0015). */
class TodoWidgetProvider : AppWidgetProvider() {
    override fun onReceive(context: Context, intent: Intent) {
        if (WidgetUpdater.isRedrawAction(intent.action)) {
            WidgetUpdater.redrawAsync(context, goAsync())
        } else {
            super.onReceive(context, intent)
        }
    }

    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        WidgetUpdater.redrawAsync(context, goAsync())
    }

    override fun onAppWidgetOptionsChanged(
        context: Context,
        manager: AppWidgetManager,
        id: Int,
        options: android.os.Bundle,
    ) {
        // The responsive RemoteViews already handle the new size; see ResizeRedraw.
        WidgetUpdater.redrawOnResize(context, goAsync())
    }

    /** A removed widget takes its settings with it. */
    override fun onDeleted(context: Context, ids: IntArray) {
        WidgetConfigStore.remove(context, ids)
    }

    /** After a backup restore the widgets come back under new ids. */
    override fun onRestored(context: Context, oldIds: IntArray, newIds: IntArray) {
        WidgetConfigStore.move(context, oldIds, newIds)
    }

    override fun onDisabled(context: Context) {
        WidgetUpdater.cancelMidnight(context)
    }
}
