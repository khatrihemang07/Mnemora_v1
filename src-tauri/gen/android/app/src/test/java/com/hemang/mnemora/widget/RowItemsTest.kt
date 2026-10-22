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
    fun isCutAtTheCapWithAMoreLineSayingHowManyAreLeftOff() {
        val rows = (1..200).map { row("t$it") }
        val items = RowItems.flatten(listOf(Group("Today", Tone.NORMAL, rows)))
        assertEquals(RowItems.MAX_ITEMS, items.size)
        assertTrue(items.first() is Item.Heading)
        // The heading and 78 Tasks stay, the More line takes the last place.
        assertEquals(Item.More(200 - 78), items.last())
        assertEquals(78, items.count { it is Item.Task })
    }

    @Test
    fun exactlyTheCapNeedsNoMoreLine() {
        val rows = (1..RowItems.MAX_ITEMS - 1).map { row("t$it") }
        val items = RowItems.flatten(listOf(Group("Today", Tone.NORMAL, rows)))
        assertEquals(RowItems.MAX_ITEMS, items.size)
        assertTrue(items.none { it is Item.More })
    }

    @Test
    fun theMoreLineNeverFollowsAHeadingWithNothingUnderIt() {
        val first = (1..RowItems.MAX_ITEMS - 3).map { row("a$it") }
        val groups = listOf(
            Group("Overdue", Tone.OVERDUE, first),
            Group("Today", Tone.NORMAL, listOf(row("b1"), row("b2"), row("b3"))),
        )
        // Overdue heading + 77 Tasks = 78 lines, then the Today heading would be line 79, then the cut.
        val items = RowItems.flatten(groups)
        assertTrue(items.last() is Item.More)
        assertTrue(items[items.size - 2] is Item.Task)
        assertEquals(3, (items.last() as Item.More).hidden)
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
        // A preview is cut for space, so it never gets the More line.
        assertTrue(RowItems.preview(groups, 2).none { it is Item.More })
    }
}
