package com.hemang.mnemora.widget

/** One line of the widget's list: a group heading or a Task. */
sealed interface Item {
    data class Heading(val text: String, val tone: Tone) : Item

    data class Task(val row: TaskRow) : Item
}

object RowItems {
    /** Most lines a RemoteViews collection should carry; the rest are left off. */
    const val MAX_ITEMS = 80

    /** The snapshot's groups as a flat list of lines, cut at [max]. */
    fun flatten(groups: List<Group>, max: Int = MAX_ITEMS): List<Item> {
        val items = ArrayList<Item>()
        for (group in groups) {
            group.heading?.let { items.add(Item.Heading(it, group.tone)) }
            for (row in group.rows) items.add(Item.Task(row))
        }
        return if (items.size > max) items.subList(0, max).toList() else items
    }

    /** The first [lines] lines for a small preview, never ending on a heading with nothing under it. */
    fun preview(groups: List<Group>, lines: Int): List<Item> {
        val items = flatten(groups, lines).toMutableList()
        while (items.lastOrNull() is Item.Heading) items.removeAt(items.size - 1)
        return items
    }
}
