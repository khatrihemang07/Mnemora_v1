package com.hemang.mnemora.widget.quickadd

import android.app.Activity
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.Selection
import android.text.TextWatcher
import android.util.TypedValue
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.WindowInsets
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.annotation.RequiresApi
import com.hemang.mnemora.R
import com.hemang.mnemora.widget.Palette
import com.hemang.mnemora.widget.WidgetBridge
import com.hemang.mnemora.widget.WidgetConfigStore
import com.hemang.mnemora.widget.WidgetRenderer
import com.hemang.mnemora.widget.WidgetTheme
import com.hemang.mnemora.widget.WidgetUpdater
import java.time.ZoneId
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger

/**
 * Quick add over the home screen: a native copy of the app's Quick add (ADR 0016), opened by the
 * widget's + and by its "Add a task..." pill. A translucent bottom sheet in the tapped widget's
 * theme, with the keyboard up. It never has rules of its own: the words are read, and the Task
 * added, by the bridge (the app's Rust rules); the pure helpers are [CaptureRules]. Enter adds,
 * closes the sheet and shows the app's toast; tapping outside or Back discards.
 */
@RequiresApi(31)
class QuickAddActivity : Activity() {
    private enum class Picker { NONE, LIST, PRIORITY }

    private val main = Handler(Looper.getMainLooper())
    private val io = Executors.newSingleThreadExecutor()
    private val sequence = AtomicInteger()

    private lateinit var palette: Palette
    private lateinit var view: String

    // What the user has decided.
    private var words = CaptureWords()
    private var hand = QuickHand()
    private var notesOpen = false
    private var justUnrecognised = false
    private var vetoed = false
    private var busy = false
    private var dismissedHashAt: Int? = null
    private var picker = Picker.NONE

    // What the bridge said.
    private var context: QuickContext? = null
    private var reading: Reading? = null
    private var readingFor = ""

    private lateinit var sheet: LinearLayout
    private lateinit var field: CaptureEditText
    private lateinit var notesField: CaptureEditText
    private lateinit var suggestionBox: LinearLayout
    private lateinit var chipsBox: FlowLayout
    private lateinit var pickerBox: LinearLayout
    private lateinit var propsBox: FlowLayout
    private lateinit var addButton: ImageView

    private val line get() = Palette.blend(palette.ink, palette.background, 0.14f)
    private val todoSoft get() = Palette.withAlpha(palette.todo, 0.14f)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val widgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
        val config = WidgetConfigStore.get(this, widgetId)
        view = intent.getStringExtra(EXTRA_VIEW)?.takeIf { com.hemang.mnemora.widget.WidgetConfig.isView(it) } ?: config.view
        val look = WidgetTheme.resolve(config.theme, WidgetConfigStore.appTheme(this), WidgetRenderer.phoneNight(this))
        palette = Palette.from(this, look)
        setFinishOnTouchOutside(false)
        window.setDecorFitsSystemWindows(false)
        window.setSoftInputMode(
            WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE or WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE,
        )
        setContentView(build())
        field.requestFocus()
        loadContext()
        requestReading()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // The keyboard comes up with the sheet, not after a tap.
        if (hasFocus && !busy) {
            field.post {
                field.requestFocus()
                (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).showSoftInput(field, InputMethodManager.SHOW_IMPLICIT)
            }
        }
    }

    override fun onDestroy() {
        io.shutdown()
        super.onDestroy()
    }

    @Suppress("OVERRIDE_DEPRECATION")
    override fun onBackPressed() = goBack()

    /** Back closes an open picker first, then throws the draft away. */
    private fun goBack() {
        if (picker != Picker.NONE) showPicker(Picker.NONE) else discard()
    }

    /** Tapping outside or Back: the draft is thrown away. */
    private fun discard() {
        if (!isFinishing) finish()
    }

    // --- Building the sheet ------------------------------------------------------------------------

    private fun dp(v: Int) = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v.toFloat(), resources.displayMetrics).toInt()

    private fun round(fill: Int, radiusDp: Int, stroke: Int? = null, dashed: Boolean = false) = GradientDrawable().apply {
        setColor(fill)
        cornerRadius = dp(radiusDp).toFloat()
        if (stroke != null) {
            if (dashed) setStroke(dp(1), stroke, dp(3).toFloat(), dp(3).toFloat()) else setStroke(dp(1), stroke)
        }
    }

    private fun text(value: CharSequence, sp: Float, color: Int, bold: Boolean = false) = TextView(this).apply {
        text = value
        setTextSize(TypedValue.COMPLEX_UNIT_SP, sp)
        setTextColor(color)
        if (bold) typeface = Typeface.DEFAULT_BOLD
    }

    private fun build(): View {
        val root = FrameLayout(this)
        root.setOnClickListener { discard() }

        sheet = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(palette.background)
                val r = dp(22).toFloat()
                cornerRadii = floatArrayOf(r, r, r, r, 0f, 0f, 0f, 0f)
            }
            isClickable = true
            setPadding(dp(16), dp(10), dp(16), 0)
        }
        val grab = View(this).apply { background = round(line, 2) }
        sheet.addView(grab, LinearLayout.LayoutParams(dp(36), dp(4)).apply {
            gravity = Gravity.CENTER_HORIZONTAL
            bottomMargin = dp(10)
        })

        // The title row: a dimmed diamond, as in the app, and the field.
        val top = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        val diamond = FrameLayout(this)
        diamond.addView(
            ImageView(this).apply {
                setImageResource(R.drawable.widget_diamond_edge)
                imageTintList = android.content.res.ColorStateList.valueOf(Palette.withAlpha(palette.faint, 0.7f))
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            },
            FrameLayout.LayoutParams(dp(22), dp(22), Gravity.CENTER),
        )
        top.addView(diamond, LinearLayout.LayoutParams(dp(30), dp(30)))
        field = CaptureEditText(this).apply {
            background = null
            setPadding(dp(8), dp(6), 0, dp(6))
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 16f)
            setTextColor(palette.ink)
            setHintTextColor(palette.faint)
            hint = getString(R.string.widget_quick_add_hint)
            highlightColor = Palette.withAlpha(palette.todo, 0.3f)
            setHighlight(Palette.withAlpha(palette.todo, 0.2f))
            textCursorDrawable = GradientDrawable().apply { setColor(palette.todo); setSize(dp(2), 0) }
            // One line, no suggestions or auto-correct: the words are read by Rust, not the keyboard.
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            setSingleLine(true)
            setHorizontallyScrolling(true)
            imeOptions = EditorInfo.IME_ACTION_DONE or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            filters = arrayOf(pasteFilter(), backspaceFilter(), InputFilter.LengthFilter(CaptureRules.MAX_TITLE))
            onCaretMoved = { showSuggestions() }
            onBack = { goBack(); true }
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
                override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
                override fun afterTextChanged(s: Editable?) = textChanged()
            })
            setOnEditorActionListener { _, _, _ -> submit(); true }
            setOnKeyListener { _, code, event -> titleKey(code, event) }
        }
        top.addView(field, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        sheet.addView(top, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))

        // Everything under the title lines up with the text, past the diamond.
        val body = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        suggestionBox = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = round(palette.surface2, 8, line)
            visibility = View.GONE
        }
        body.addView(suggestionBox, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(4); bottomMargin = dp(2) })

        notesField = CaptureEditText(this).apply {
            onBack = { goBack(); true }
            background = null
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14.4f)
            setTextColor(palette.muted)
            setHintTextColor(palette.faint)
            hint = getString(R.string.widget_quick_add_notes_hint)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            minLines = 2
            gravity = Gravity.TOP
            setPadding(0, dp(8), 0, dp(4))
            visibility = View.GONE
            setOnKeyListener { _, code, event -> notesKey(code, event) }
        }
        body.addView(notesField, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(4) })

        chipsBox = FlowLayout(this, dp(6), dp(6)).apply { visibility = View.GONE }
        body.addView(chipsBox, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(6) })

        pickerBox = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = round(palette.surface2, 10, line)
            visibility = View.GONE
        }
        body.addView(pickerBox, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(8) })

        val rule = View(this).apply { setBackgroundColor(line) }
        body.addView(rule, LinearLayout.LayoutParams(MATCH_PARENT, dp(1)).apply { topMargin = dp(8) })
        propsBox = FlowLayout(this, dp(6), dp(6))
        body.addView(propsBox, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(8) })
        sheet.addView(body, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { marginStart = dp(30) })

        // Adding by tap, for a keyboard without an Enter that adds.
        // A close and a submit button, right-aligned: the same pair as the app's capture box.
        val closeButton = ImageView(this).apply {
            setImageResource(R.drawable.qa_x)
            imageTintList = android.content.res.ColorStateList.valueOf(palette.faint)
            setPadding(dp(9), dp(9), dp(9), dp(9))
            contentDescription = getString(R.string.widget_quick_add_close)
            setOnClickListener { discard() }
        }
        addButton = ImageView(this).apply {
            setImageResource(R.drawable.qa_arrow_up)
            imageTintList = android.content.res.ColorStateList.valueOf(palette.onTodo)
            background = round(palette.todo, 18)
            setPadding(dp(9), dp(9), dp(9), dp(9))
            contentDescription = getString(R.string.widget_quick_add_add)
            setOnClickListener { if (!field.text.isNullOrBlank() && !busy) submit() }
        }
        val actions = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(closeButton, LinearLayout.LayoutParams(dp(36), dp(36)))
            addView(addButton, LinearLayout.LayoutParams(dp(36), dp(36)).apply { marginStart = dp(4) })
        }
        sheet.addView(actions, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply {
            gravity = Gravity.END
            topMargin = dp(8)
            bottomMargin = dp(10)
        })

        root.addView(sheet, FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.BOTTOM))
        // The sheet reaches the screen's bottom edge and lifts its content over the keyboard and the
        // navigation bar, so no strip of home shows underneath it.
        root.setOnApplyWindowInsetsListener { _, insets ->
            val bottom = maxOf(
                insets.getInsets(WindowInsets.Type.ime()).bottom,
                insets.getInsets(WindowInsets.Type.systemBars()).bottom,
            )
            sheet.setPadding(dp(16), dp(10), dp(16), bottom)
            insets
        }
        renderProps()
        updateAddButton()
        return root
    }

    // --- Talking to the bridge ---------------------------------------------------------------------

    private fun bridge(request: String): String? = WidgetBridge.call(applicationContext, request)

    private fun loadContext() {
        val request = QuickAddRequests.context(view, System.currentTimeMillis(), ZoneId.systemDefault())
        io.execute {
            val reply = QuickAddReplies.context(bridge(request))
            main.post { if (!isDestroyed) onContext(reply) }
        }
    }

    private fun onContext(reply: QuickResult<QuickContext>) {
        when (reply) {
            is QuickResult.Ok -> {
                context = reply.value
                field.hint = reply.value.placeholder
                showSuggestions()
            }
            else -> {
                // Not set up, needs an update, or broken: nothing to add to. Say so and close.
                toast(failureText(reply))
                finish()
            }
        }
    }

    private fun failureText(reply: QuickResult<*>): String = when (reply) {
        QuickResult.NotSetUp -> getString(R.string.widget_not_set_up_title) + " " + getString(R.string.widget_not_set_up_line)
        QuickResult.NeedsUpdate -> getString(R.string.widget_needs_update_title)
        is QuickResult.Failed -> reply.message ?: getString(R.string.widget_quick_add_failed)
        is QuickResult.Ok -> getString(R.string.widget_quick_add_failed)
    }

    private fun toast(message: String) {
        Toast.makeText(applicationContext, message, Toast.LENGTH_SHORT).show()
    }

    /** Asks Rust what the text means; a newer ask makes the answer to an older one irrelevant. */
    private fun requestReading() {
        val text = field.text.toString()
        val draft = QuickDraft(view, text, words = words, hand = hand)
        val mine = sequence.incrementAndGet()
        io.execute {
            if (mine != sequence.get()) return@execute
            val request = QuickAddRequests.parse(draft, System.currentTimeMillis(), ZoneId.systemDefault())
            val reply = QuickAddReplies.reading(bridge(request))
            main.post { if (mine == sequence.get() && !isDestroyed) onReading(reply, text) }
        }
    }

    private fun onReading(reply: QuickResult<Reading>, text: String) {
        val got = (reply as? QuickResult.Ok)?.value
        reading = got
        readingFor = text
        if (got != null) {
            field.setMarks(got.spans.map { TextSpan(it.start, it.end) }, text)
            // Decisions about words that are gone from the text are forgotten.
            val pruned = CaptureRules.pruneWords(words, (got.spans + got.alternatives + got.ignored).map { it.key })
            if (pruned != words) {
                words = pruned
                requestReading()
            }
        } else {
            field.setMarks(emptyList(), text)
        }
        renderChips()
        renderProps()
    }

    // --- Typing ------------------------------------------------------------------------------------

    private fun textChanged() {
        if (vetoed) vetoed = false else justUnrecognised = false
        dismissedHashAt = dismissedHashAt?.takeIf { hashAtCaret()?.start == it }
        updateAddButton()
        showSuggestions()
        requestReading()
    }

    private fun updateAddButton() {
        addButton.alpha = if (field.text.isNullOrBlank() || busy) 0.4f else 1f
    }

    /** Enter adds, Shift+Enter opens the notes. Anything else is the field's. */
    private fun titleKey(code: Int, event: KeyEvent): Boolean {
        if (code != KeyEvent.KEYCODE_ENTER && code != KeyEvent.KEYCODE_NUMPAD_ENTER) return false
        if (event.action == KeyEvent.ACTION_DOWN && event.repeatCount == 0) {
            if (event.isShiftPressed) openNotes() else submit()
        }
        return true
    }

    private fun notesKey(code: Int, event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false
        // Ctrl or Meta + Enter adds from the notes; Backspace on empty notes closes them.
        if (code == KeyEvent.KEYCODE_ENTER && (event.isCtrlPressed || event.isMetaPressed)) {
            submit()
            return true
        }
        if (code == KeyEvent.KEYCODE_DEL && notesField.text.isEmpty()) {
            closeNotes()
            return true
        }
        return false
    }

    /**
     * A multi-line paste is one Task: the first line is the title, the rest the notes. Runs for the
     * menu's Paste and for a keyboard's clipboard strip alike, because it is a filter on the text.
     */
    private fun pasteFilter() = InputFilter { source, start, end, _, _, _ ->
        val inserted = source.subSequence(start, end).toString()
        if (inserted.none { it == '\n' || it == '\r' }) return@InputFilter null
        val parts = CaptureRules.pasteParts(inserted)
            ?: return@InputFilter inserted.replace(Regex("[\r\n]+"), "")
        if (parts.notes.isNotEmpty()) {
            main.post {
                val have = notesField.text.toString()
                notesField.setText(if (have.isEmpty()) parts.notes else have + "\n" + parts.notes)
                openNotes(focusNotes = false)
            }
        }
        parts.title
    }

    /** ⌫ right after a recognised word keeps it as text instead of deleting a letter. */
    private fun backspaceFilter() = InputFilter { source, start, end, dest, dstart, dend ->
        val now = reading
        if (source.subSequence(start, end).isNotEmpty() || dend - dstart != 1 || now == null) return@InputFilter null
        val text = dest.toString()
        if (text != readingFor || Selection.getSelectionStart(dest) != dend || Selection.getSelectionEnd(dest) != dend) {
            return@InputFilter null
        }
        val target = CaptureRules.backspaceTarget(
            text, dend, dend, now.spans.map { SpanEnd(it.key, it.end) }, justUnrecognised,
        ) ?: return@InputFilter null
        vetoed = true
        justUnrecognised = true
        words = CaptureRules.ignoreWord(words, target)
        // At once, so a second ⌫ before Rust answers deletes as usual.
        val word = now.spans.first { it.key == target }
        reading = now.copy(spans = now.spans - word, ignored = now.ignored + word)
        main.post {
            field.setMarks(reading?.spans.orEmpty().map { TextSpan(it.start, it.end) }, readingFor)
            field.setSelection(minOf(dend, field.text.length))
            renderChips()
        }
        dest.subSequence(dstart, dend)
    }

    // --- #List suggestions -------------------------------------------------------------------------

    private fun hashAtCaret(): HashQuery? = try {
        CaptureRules.hashQuery(field.text.toString(), field.selectionStart)
    } catch (e: Throwable) {
        null
    }

    /** Never throws: a failure in the rules means no suggestions, not a crash. */
    private fun currentSuggestions(): List<Suggestion> = try {
        suggestionsFor()
    } catch (e: Throwable) {
        android.util.Log.w("TodoWidget", "suggestions failed", e)
        emptyList()
    }

    private fun suggestionsFor(): List<Suggestion> {
        val ctx = context ?: return emptyList()
        if (!ctx.parseDates) return emptyList()
        val text = field.text.toString()
        val hash = hashAtCaret() ?: return emptyList()
        if (hash.start == dismissedHashAt) return emptyList()
        return CaptureRules.listSuggestions(hash.query, ctx.lists.map { SuggestList(it.id, it.name, it.emoji) })
    }

    private fun showSuggestions() {
        if (!::suggestionBox.isInitialized) return
        val items = currentSuggestions().take(5)
        suggestionBox.removeAllViews()
        suggestionBox.visibility = if (items.isEmpty()) View.GONE else View.VISIBLE
        items.forEachIndexed { i, s ->
            val label = when (s) {
                is Suggestion.Existing -> listOfNotNull(s.emoji?.takeIf { it.isNotEmpty() }, s.name).joinToString(" ")
                is Suggestion.New -> getString(R.string.widget_quick_add_new_list, s.name)
            }
            val ink = if (s is Suggestion.New) palette.todo else palette.ink
            // The app's glyph: an emoji in the label, else the list icon; a new List has list-plus.
            val glyph = when {
                s is Suggestion.New -> R.drawable.qa_list_plus
                (s as Suggestion.Existing).emoji.isNullOrEmpty() -> R.drawable.qa_list
                else -> null
            }
            val row = text(label, 14f, ink).apply {
                gravity = Gravity.CENTER_VERTICAL
                compoundDrawablePadding = dp(8)
                if (glyph != null) setCompoundDrawablesRelative(icon(glyph, if (s is Suggestion.New) palette.todo else palette.muted).apply { setBounds(0, 0, dp(16), dp(16)) }, null, null, null)
                setPadding(dp(12), 0, dp(12), 0)
                maxLines = 1
                ellipsize = android.text.TextUtils.TruncateAt.END
                // The first row is what Enter takes, as in the app.
                if (i == 0) setBackgroundColor(Palette.withAlpha(palette.ink, 0.06f))
                setOnClickListener { accept(s) }
            }
            suggestionBox.addView(row, LinearLayout.LayoutParams(MATCH_PARENT, dp(40)))
        }
    }

    private fun accept(s: Suggestion) {
        val text = field.text.toString()
        val hash = hashAtCaret() ?: return
        val (next, caret) = CaptureRules.applySuggestion(text, hash, s)
        field.setText(next)
        field.setSelection(minOf(caret, field.text.length))
    }

    // --- Chips -------------------------------------------------------------------------------------

    private fun icon(res: Int, color: Int): android.graphics.drawable.Drawable =
        getDrawable(res)!!.mutate().apply {
            setTint(color)
            setBounds(0, 0, dp(14), dp(14))
        }

    /** A chip with the app's icon before its label and, when it opens something, a chevron after. */
    private fun chip(
        label: CharSequence,
        fill: Int,
        ink: Int,
        stroke: Int? = null,
        dashed: Boolean = false,
        iconRes: Int? = null,
        chevron: Boolean = false,
    ) = text(label, 12f, ink).apply {
        background = round(fill, 14, stroke, dashed)
        setPadding(dp(10), dp(5), dp(10), dp(5))
        maxLines = 1
        ellipsize = android.text.TextUtils.TruncateAt.END
        maxWidth = dp(260)
        gravity = Gravity.CENTER_VERTICAL
        compoundDrawablePadding = dp(4)
        if (iconRes != null || chevron) {
            setCompoundDrawablesRelative(
                iconRes?.let { icon(it, ink) },
                null,
                if (chevron) icon(R.drawable.qa_chevron, Palette.withAlpha(ink, 0.7f)) else null,
                null,
            )
        }
    }

    /** The "word → meaning ×" chips, then the alternatives and the words kept as text. */
    private fun renderChips() {
        chipsBox.removeAllViews()
        val r = reading
        if (r == null || (r.spans.isEmpty() && r.alternatives.isEmpty() && r.ignored.isEmpty())) {
            chipsBox.visibility = View.GONE
            return
        }
        chipsBox.visibility = View.VISIBLE
        for (s in r.spans) {
            val box = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                background = round(todoSoft, 14)
            }
            val label = text("${s.word} → ${s.meaning}", 12f, palette.todo).apply {
                maxLines = 1
                ellipsize = android.text.TextUtils.TruncateAt.END
                maxWidth = dp(220)
                setPadding(dp(10), dp(5), 0, dp(5))
            }
            val cross = text("×", 15f, palette.todo, bold = true).apply {
                gravity = Gravity.CENTER
                setPadding(dp(8), dp(1), dp(10), dp(3))
                contentDescription = getString(R.string.widget_quick_add_keep_as_text, s.word)
                setOnClickListener {
                    words = CaptureRules.ignoreWord(words, s.key)
                    requestReading()
                }
            }
            box.addView(label)
            box.addView(cross, LinearLayout.LayoutParams(WRAP_CONTENT, dp(28)))
            chipsBox.addView(box)
        }
        for (s in r.alternatives) {
            chipsBox.addView(
                chip("${s.word} → ${s.meaning} · ${getString(R.string.widget_quick_add_use_instead)}", 0, palette.faint, line, dashed = true).apply {
                    setOnClickListener {
                        words = CaptureRules.preferWord(words, WordRef(s.kind, s.key))
                        requestReading()
                    }
                },
            )
        }
        for (s in r.ignored) {
            chipsBox.addView(
                chip("“${s.word}” kept as text ↺", 0, palette.faint, line, dashed = true).apply {
                    setOnClickListener {
                        words = CaptureRules.recogniseAgain(words, s.key)
                        requestReading()
                    }
                },
            )
        }
    }

    private fun priorityInk(p: Int) = when (p) {
        1 -> palette.danger
        2 -> palette.warning
        3 -> palette.accent
        else -> palette.ink
    }

    /** List, When (read-only: there is no When picker here), Priority and Notes. */
    private fun renderProps() {
        propsBox.removeAllViews()
        val r = reading ?: return
        val listChosen = r.listIsNew || r.fields.listId != null
        val listEmoji = context?.lists?.firstOrNull { it.id == r.fields.listId }?.emoji
        // The app's glyph: the List's emoji (already in the label), else the list icon; Inbox and a new List have their own.
        val listIcon = when {
            r.listIsNew -> R.drawable.qa_list_plus
            r.fields.listId == null -> R.drawable.qa_inbox
            listEmoji.isNullOrEmpty() -> R.drawable.qa_list
            else -> null
        }
        propsBox.addView(
            (if (listChosen) chip(r.listLabel, todoSoft, palette.todo, iconRes = listIcon, chevron = true) else chip(r.listLabel, 0, palette.ink, line, iconRes = listIcon, chevron = true)).apply {
                contentDescription = "List: ${r.listLabel}. Choose a List"
                setOnClickListener { showPicker(if (picker == Picker.LIST) Picker.NONE else Picker.LIST) }
            },
        )
        val dated = r.fields.date != null || r.fields.time != null
        // Not tappable, so no chevron: the calendar, time and Repeat panels are not in the sheet.
        val whenIcon = if (r.fields.hasRepeat) R.drawable.qa_repeat else R.drawable.qa_calendar
        propsBox.addView(
            (if (dated) chip(r.whenLabel, todoSoft, palette.todo, iconRes = whenIcon) else chip(r.whenLabel, 0, palette.faint, line, iconRes = whenIcon)).apply {
                contentDescription = "When: ${r.whenLabel}"
            },
        )
        val p = r.fields.priority
        propsBox.addView(
            (if (p < 4) chip("P$p", Palette.withAlpha(priorityInk(p), 0.14f), priorityInk(p), iconRes = R.drawable.qa_flag, chevron = true) else chip(getString(R.string.widget_quick_add_priority), 0, palette.ink, line, iconRes = R.drawable.qa_flag, chevron = true)).apply {
                contentDescription = "Priority: ${priorityName(p)}"
                setOnClickListener { showPicker(if (picker == Picker.PRIORITY) Picker.NONE else Picker.PRIORITY) }
            },
        )
        if (!notesOpen) {
            propsBox.addView(
                chip(getString(R.string.widget_quick_add_notes), 0, palette.ink, line, iconRes = R.drawable.qa_note).apply {
                    contentDescription = "Add notes"
                    setOnClickListener { openNotes() }
                },
            )
        }
    }

    private fun priorityName(p: Int) = getString(
        when (p) {
            1 -> R.string.widget_quick_add_priority_1
            2 -> R.string.widget_quick_add_priority_2
            3 -> R.string.widget_quick_add_priority_3
            else -> R.string.widget_quick_add_priority_4
        },
    )

    // --- Pickers -----------------------------------------------------------------------------------

    private fun pickerRow(label: String, ink: Int, checked: Boolean, onPick: () -> Unit) =
        text(if (checked) "$label  ✓" else label, 14f, ink).apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(14), 0, dp(14), 0)
            maxLines = 1
            ellipsize = android.text.TextUtils.TruncateAt.END
            setOnClickListener { onPick() }
        }

    private fun showPicker(which: Picker) {
        picker = which
        pickerBox.removeAllViews()
        if (which == Picker.NONE) {
            pickerBox.visibility = View.GONE
            return
        }
        val r = reading
        val rows = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        if (which == Picker.LIST) {
            val current = r?.fields?.listId
            rows.addView(
                pickerRow(getString(R.string.widget_quick_add_inbox), palette.ink, current == null && r?.listIsNew != true) { pickList(null) },
                LinearLayout.LayoutParams(MATCH_PARENT, dp(44)),
            )
            for (l in context?.lists.orEmpty()) {
                rows.addView(pickerRow(l.label, palette.ink, current == l.id) { pickList(l.id) }, LinearLayout.LayoutParams(MATCH_PARENT, dp(44)))
            }
        } else {
            val current = r?.fields?.priority ?: 4
            for (p in 1..4) {
                rows.addView(pickerRow(priorityName(p), priorityInk(p), current == p) { pickPriority(p) }, LinearLayout.LayoutParams(MATCH_PARENT, dp(44)))
            }
        }
        val scroll = ScrollView(this).apply { isVerticalScrollBarEnabled = false; addView(rows) }
        pickerBox.addView(scroll, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
        pickerBox.visibility = View.VISIBLE
        // A scroll view taller than a few rows would push the field off screen over the keyboard.
        pickerBox.post {
            val max = dp(190)
            if (scroll.height > max) scroll.layoutParams = scroll.layoutParams.apply { height = max }
        }
    }

    /** A chip used by hand is a decision: the typed words of its kind become text, so the two agree. */
    private fun decided(kind: String) {
        val r = reading
        val found = (r?.spans.orEmpty() + r?.alternatives.orEmpty()).map { WordRef(it.kind, it.key) }
        words = CaptureRules.ignoreKinds(words, found, listOf(kind))
    }

    private fun pickList(id: String?) {
        hand = hand.copy(listTouched = true, listId = id)
        decided("list")
        showPicker(Picker.NONE)
        requestReading()
    }

    private fun pickPriority(p: Int) {
        hand = hand.copy(priority = p)
        decided("prio")
        showPicker(Picker.NONE)
        requestReading()
    }

    // --- Notes -------------------------------------------------------------------------------------

    private fun openNotes(focusNotes: Boolean = true) {
        notesOpen = true
        notesField.visibility = View.VISIBLE
        renderProps()
        if (focusNotes) notesField.requestFocus()
    }

    private fun closeNotes() {
        notesOpen = false
        notesField.visibility = View.GONE
        renderProps()
        field.requestFocus()
        field.setSelection(field.text.length)
    }

    // --- Adding ------------------------------------------------------------------------------------

    private fun submit() {
        if (busy) return
        // While suggestions show, Enter takes the first one and never adds the Task.
        val suggestions = currentSuggestions()
        if (suggestions.isNotEmpty()) {
            accept(suggestions.first())
            return
        }
        val text = field.text.toString()
        if (text.isBlank()) return
        busy = true
        updateAddButton()
        val draft = QuickDraft(view, text, notesField.text.toString(), words, hand)
        val app = applicationContext
        io.execute {
            val request = QuickAddRequests.add(draft, System.currentTimeMillis(), ZoneId.systemDefault())
            val reply = QuickAddReplies.added(WidgetBridge.call(app, request))
            main.post { added(reply) }
        }
    }

    private fun added(reply: QuickResult<Added>) {
        if (reply is QuickResult.Ok) {
            toast(reply.value.message)
            // The row appears in every Todo widget; a running app is told by the bridge.
            WidgetUpdater.redrawAsync(applicationContext, null)
            if (!isFinishing) finish()
            return
        }
        // Nothing was added: say why and keep what was typed.
        busy = false
        updateAddButton()
        toast(failureText(reply))
        if (reply !is QuickResult.Failed && !isFinishing) finish()
    }

    companion object {
        private const val EXTRA_VIEW = "view"

        /** The sheet for the widget showing [view]. A fresh task each time, so a stale draft is never reopened. */
        fun intent(context: Context, view: String, widgetId: Int): Intent =
            Intent(context, QuickAddActivity::class.java)
                .putExtra(EXTRA_VIEW, view)
                .putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)

        /** What a tap on the widget's + or pill runs. Explicit and immutable, one per widget. */
        fun pending(context: Context, view: String, widgetId: Int): PendingIntent =
            PendingIntent.getActivity(
                context,
                REQUEST_BASE + widgetId,
                intent(context, view, widgetId),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )

        private const val REQUEST_BASE = 0x51410000
    }
}
