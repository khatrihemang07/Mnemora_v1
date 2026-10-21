package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RowItemsTest {
    private fun row(id: String) = TaskRow(id, "Task $id", "none", false, null, null, null)

    @Test
    fun headingsPrecedeTheirRows() {
        val groups = listOf(
            Group("Overdue", Tone.OVERDUE, listOf(row("a"))),
            Group("Today", Tone.NORMAL, listOf(row("b"), row("c"))),
        )
        val items = RowItems.flatten(groups)
        assertEquals(5, items.size)
        assertEquals(Item.Heading("Overdue", Tone.OVERDUE), items[0])
        assertEquals(Item.Task(row("a")), items[1])
        assertEquals(Item.Heading("Today", Tone.NORMAL), items[2])
        assertEquals(Item.Task(row("c")), items[4])
    }

    @Test
    fun aGroupWithoutAHeadingAddsOnlyRows() {
        val items = RowItems.flatten(listOf(Group(null, Tone.NORMAL, listOf(row("a")))))
        assertEquals(listOf<Item>(Item.Task(row("a"))), items)
    }

    @Test
    fun isCutAtTheCap() {
        val rows = (1..200).map { row("t$it") }
        val items = RowItems.flatten(listOf(Group("Today", Tone.NORMAL, rows)))
        assertEquals(RowItems.MAX_ITEMS, items.size)
        assertTrue(items.first() is Item.Heading)
    }

    @Test
    fun aPreviewNeverEndsOnAHeading() {
        val groups = listOf(
            Group("Overdue", Tone.OVERDUE, listOf(row("a"), row("b"))),
            Group("Today", Tone.NORMAL, listOf(row("c"))),
        )
        // Cut after "Overdue, a, b": the next line would be the Today heading, which is dropped.
        assertEquals(3, RowItems.preview(groups, 3).size)
        assertEquals(3, RowItems.preview(groups, 4).size)
        assertEquals(5, RowItems.preview(groups, 5).size)
        assertTrue(RowItems.preview(groups, 5).last() is Item.Task)
    }
}
