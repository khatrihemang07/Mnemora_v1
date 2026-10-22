package com.hemang.mnemora.widget.quickadd

import java.text.Normalizer

/**
 * The Quick add sheet's pure rules: a Kotlin copy of the app's Capture box helpers in
 * `src/modules/todo/capture.ts` (ADR 0016). `src/lib/ipc/fixtures/capture-ui.json` runs the same
 * cases against both. Reading the words stays in Rust; offsets are UTF-16 units, which is what a
 * Kotlin `String` index is.
 */

/** What the user has decided about the words the parser found. */
data class CaptureWords(
    /** Keys of words kept as text. */
    val ignored: List<String> = emptyList(),
    /** Per kind, the key of the word chosen over the last one. */
    val prefer: Map<String, String> = emptyMap(),
)

/** A word the parser found, as far as the rules care: its kind and its key. */
data class WordRef(val kind: String, val key: String)

/** Where a word ends in the text. */
data class SpanEnd(val key: String, val end: Int)

/** A recognised word's place in the text: [start] up to, not including, [end]. */
data class TextSpan(val start: Int, val end: Int)

/** A stretch of the text that is, or is not, a recognised word. */
data class Segment(val text: String, val mark: Boolean)

/** The `#word` the caret ends. */
data class HashQuery(val start: Int, val query: String)

/** A List that `#` can complete to. */
data class SuggestList(val id: String, val name: String, val emoji: String?)

/** A row in the `#` suggestions: a List, or the offer to make a new one. */
sealed interface Suggestion {
    data class Existing(val id: String, val name: String, val emoji: String?) : Suggestion

    data class New(val name: String) : Suggestion
}

/** A multi-line paste: the first line is the title, the rest the notes. */
data class PasteParts(val title: String, val notes: String)

object CaptureRules {
    /** Longest title; Rust enforces it too. */
    const val MAX_TITLE = 500

    // --- Words kept as text, and "use instead" ---------------------------------------------------

    /** Keeps a word as text (× on its chip, or ⌫ right after it). */
    fun ignoreWord(words: CaptureWords, key: String): CaptureWords {
        if (key in words.ignored) return words
        return CaptureWords(words.ignored + key, words.prefer.filterValues { it != key })
    }

    /** Reads a word again (the "kept as text ↺" chip). */
    fun recogniseAgain(words: CaptureWords, key: String): CaptureWords =
        words.copy(ignored = words.ignored.filter { it != key })

    /** Chooses a word over the one that would win ("use instead"). */
    fun preferWord(words: CaptureWords, span: WordRef): CaptureWords =
        words.copy(prefer = words.prefer + (span.kind to span.key))

    /**
     * The user set these [kinds] by hand (a property chip): the words of those kinds in the text
     * become text, so the choice is not overruled by them, and any "use instead" for them is
     * forgotten. [found] are the winning words, then the losing ones.
     */
    fun ignoreKinds(words: CaptureWords, found: List<WordRef>, kinds: List<String>): CaptureWords {
        val keys = found.filter { it.kind in kinds }.map { it.key }
        val prefer = words.prefer.filterKeys { it !in kinds }
        return CaptureWords((words.ignored + keys).distinct(), prefer)
    }

    /** Words that no longer appear in the text cannot be told apart from new ones: forget them. */
    fun pruneWords(words: CaptureWords, knownKeys: List<String>): CaptureWords {
        val known = knownKeys.toSet()
        val ignored = words.ignored.filter { it in known }
        val prefer = words.prefer.filterValues { it in known }
        return if (ignored.size == words.ignored.size && prefer.size == words.prefer.size) {
            words
        } else {
            CaptureWords(ignored, prefer)
        }
    }

    // --- ⌫ right after a recognised word ---------------------------------------------------------

    /**
     * The word ⌫ should keep as text instead of deleting a character: the caret is right after it,
     * or after the one space that follows it. The first ⌫ un-recognises; the second deletes as
     * usual. Only with a caret, never with a selection.
     */
    fun backspaceTarget(
        text: String,
        selectionStart: Int,
        selectionEnd: Int,
        spans: List<SpanEnd>,
        justUnrecognised: Boolean,
    ): String? {
        if (justUnrecognised || selectionStart != selectionEnd) return null
        val caret = selectionStart
        return spans.firstOrNull {
            it.end == caret || (it.end == caret - 1 && text.getOrNull(caret - 1) == ' ')
        }?.key
    }

    // --- #List suggestions -----------------------------------------------------------------------

    private val TAG_CHAR = Regex("[\\p{L}\\p{N}_-]")
    private val MARKS = Regex("\\p{M}")
    private val FOLD_EXTRA = mapOf('đ' to "d", 'ø' to "o", 'ł' to "l", 'ı' to "i")

    // JavaScript's \s, spelled out: Java's \s is ASCII only, Android's (ICU) is Unicode, and neither
    // accepts the other's flags, so one explicit class keeps the JVM test and the phone alike.
    private const val JS_SPACE = "\\t\\n\\u000B\\f\\r \\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000\\ufeff"
    private val HASH_AT_END = Regex("(^|[$JS_SPACE])#([\\p{L}\\p{N}_-]*)\\z")
    private val SPACE = Regex("[$JS_SPACE]")

    /** Lowercase with accents removed, as Rust folds a List name. */
    fun foldTag(s: String): String {
        val plain = Normalizer.normalize(s, Normalizer.Form.NFD).replace(MARKS, "").lowercase()
        return buildString { plain.forEach { append(FOLD_EXTRA[it] ?: it.toString()) } }
    }

    /** The part of a List name a `#tag` can spell. */
    fun tagOf(name: String): String = buildString {
        var i = 0
        while (i < name.length) {
            val cp = name.codePointAt(i)
            val ch = String(Character.toChars(cp))
            if (TAG_CHAR.matches(ch)) append(ch)
            i += Character.charCount(cp)
        }
    }

    private fun tagKey(name: String): String = foldTag(tagOf(name))

    /** The `#tag` being typed at the caret, or none. The caret must be at the end of the word. */
    fun hashQuery(text: String, caret: Int): HashQuery? {
        if (caret < 0 || caret > text.length) return null
        val m = HASH_AT_END.find(text.substring(0, caret)) ?: return null
        val next = text.getOrNull(caret)
        if (next != null && !SPACE.matches(next.toString())) return null
        val query = m.groupValues[2]
        return HashQuery(caret - query.length - 1, query)
    }

    /**
     * The Lists whose names start with what was typed (exact names first), or, when none does, the
     * offer of a new List. A bare `#`, and a tag of digits only (an issue number), suggest nothing
     * new.
     */
    fun listSuggestions(query: String, lists: List<SuggestList>): List<Suggestion> {
        val typed = foldTag(query)
        val matching = lists.filter { tagKey(it.name).startsWith(typed) }
        if (matching.isNotEmpty()) {
            val (exact, rest) = matching.partition { tagKey(it.name) == typed }
            return (exact + rest).map { Suggestion.Existing(it.id, it.name, it.emoji) }
        }
        if (query.isEmpty() || query.all { it in '0'..'9' }) return emptyList()
        return listOf(Suggestion.New(query))
    }

    /** The text with the `#tag` completed to the suggestion and a space after it, and the caret. */
    fun applySuggestion(text: String, hash: HashQuery, suggestion: Suggestion): Pair<String, Int> {
        val name = when (suggestion) {
            is Suggestion.Existing -> suggestion.name
            is Suggestion.New -> suggestion.name
        }
        val tag = tagOf(name).ifEmpty { hash.query }
        val before = text.substring(0, hash.start)
        val after = text.substring(minOf(text.length, hash.start + 1 + hash.query.length))
        val spaced = if (after.startsWith(" ")) after else " $after"
        val done = "$before#$tag"
        return Pair(done + spaced, done.length + 1)
    }

    // --- Highlighting ----------------------------------------------------------------------------

    /** The text cut into plain and recognised stretches, for the highlight behind the field. */
    fun markSegments(text: String, spans: List<TextSpan>): List<Segment> {
        val out = ArrayList<Segment>()
        var at = 0
        for ((start, end) in spans.sortedBy { it.start }) {
            if (start < at || end > text.length) continue
            if (start > at) out.add(Segment(text.substring(at, start), false))
            out.add(Segment(text.substring(start, end), true))
            at = end
        }
        if (at < text.length) out.add(Segment(text.substring(at), false))
        return out
    }

    // --- Pasting ---------------------------------------------------------------------------------

    /**
     * A multi-line paste is one Task: the first line is the title and the rest are its notes. A
     * paste with no line break is left to the field. A title over the limit spills into the notes.
     */
    fun pasteParts(pasted: String): PasteParts? {
        val lines = pasted.replace(Regex("\r\n?"), "\n").trimEnd('\n').split('\n')
        if (lines.size < 2) return null
        val first = lines.indexOfFirst { it.isNotBlank() }
        if (first < 0) return null
        val head = lines[first].trim()
        val points = head.codePointCount(0, head.length)
        val cut = if (points > MAX_TITLE) head.offsetByCodePoints(0, MAX_TITLE) else head.length
        val title = head.substring(0, cut)
        val spill = head.substring(cut)
        val rest = lines.drop(first + 1).joinToString("\n")
        val notes = listOf(spill, rest).filter { it.isNotEmpty() }.joinToString("\n").trim()
        return PasteParts(title, notes)
    }
}
