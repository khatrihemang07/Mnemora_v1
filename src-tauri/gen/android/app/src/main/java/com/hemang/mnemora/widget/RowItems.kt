package com.hemang.mnemora.widget

/** One line of the widget's list: a group heading or a Task. */
sealed interface Item {
    data class Heading(val text: String, val tone: Tone) : Item

    data class Task(val row: TaskRow) : Item

    /** The last line of a list that was cut: [hidden] more Tasks, and a tap opens the view. */
    data class More(val hidden: Int) : Item
}

object RowItems {
    /** Most lines a RemoteViews collection should carry, the More line included. */
    const val MAX_ITEMS = 80

    private fun lines(groups: List<Group>): List<Item> {
        val items = ArrayList<Item>()
        for (group in groups) {
            group.heading?.let { items.add(Item.Heading(it, group.tone)) }
            for (row in group.rows) items.add(Item.Task(row))
        }
        return items
    }

    /**
     * The snapshot's groups as a flat list of lines. Past [max] lines the list is cut and ends with
     * a [Item.More] line counting the Tasks left off, so nothing disappears without a word.
     */
    fun flatten(groups: List<Group>, max: Int = MAX_ITEMS): List<Item> {
        val items = lines(groups)
        if (items.size <= max) return items
        val kept = items.subList(0, max - 1).toMutableList()
        while (kept.lastOrNull() is Item.Heading) kept.removeAt(kept.size - 1)
        kept.add(Item.More(items.count { it is Item.Task } - kept.count { it is Item.Task }))
        return kept
    }

    /** The first [lines] lines for a small preview, never ending on a heading with nothing under it. */
    fun preview(groups: List<Group>, lines: Int): List<Item> {
        val items = lines(groups).take(lines).toMutableList()
        while (items.lastOrNull() is Item.Heading) items.removeAt(items.size - 1)
        return items
    }
}
