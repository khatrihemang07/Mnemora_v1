package com.hemang.mnemora.widget

/**
 * Built-in sample Tasks for previews. Never the user's data: the setup screen falls back to these
 * when it has nothing real to show, and the picker's static preview layout mirrors them.
 */
object SampleSnapshot {
    fun today(appTheme: String? = null) = Snapshot(
        view = "today",
        title = "Today",
        count = HeaderCount("1 overdue", Tone.OVERDUE),
        groups = listOf(
            Group(
                "Overdue", Tone.OVERDUE,
                listOf(TaskRow("sample-1", "Renew passport photos", "p2", true, null, "Mon", null)),
            ),
            Group(
                "Today", Tone.NORMAL,
                listOf(
                    TaskRow("sample-2", "Stand-up notes", "none", false, "09:30", null, null),
                    TaskRow("sample-3", "Pay electricity bill", "p1", false, null, null, "Home"),
                    TaskRow("sample-4", "Call mum", "p3", false, "18:00", null, null),
                    TaskRow("sample-5", "Water the plants", "none", false, null, null, null),
                ),
            ),
        ),
        empty = null,
        appTheme = appTheme,
    )
}
