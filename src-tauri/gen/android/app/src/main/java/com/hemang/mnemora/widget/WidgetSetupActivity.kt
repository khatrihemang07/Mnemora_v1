package com.hemang.mnemora.widget

import android.app.Activity
import android.appwidget.AppWidgetHostView
import android.appwidget.AppWidgetManager
import android.content.Intent
import android.content.res.ColorStateList
import android.content.res.Configuration
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.Button
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.ScrollView
import android.widget.TextView
import androidx.annotation.RequiresApi
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.hemang.mnemora.R
import java.time.ZoneId
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

/**
 * The Todo widget's setup screen: opens when a widget is dropped and again from the launcher's
 * reconfigure. View, Theme and Background, with a live preview drawn by the widget's own renderer.
 * Dressed in the app's theme. Backing out or Cancel on first placement removes the widget.
 */
@RequiresApi(31)
class WidgetSetupActivity : Activity() {
    private var appWidgetId = AppWidgetManager.INVALID_APPWIDGET_ID
    private var config = WidgetConfig()
    private var choices: ListChoices? = null
    private lateinit var palette: Palette
    private lateinit var previewHost: FrameLayout
    private val radios = ArrayList<Pair<RadioButton, () -> Boolean>>()
    private val replies = HashMap<String, Reply>()
    private val io: ExecutorService = Executors.newSingleThreadExecutor()
    private var previewSeq = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        appWidgetId = intent?.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
            ?: AppWidgetManager.INVALID_APPWIDGET_ID
        // Backing out is a cancel: on first placement the launcher then removes the widget.
        setResult(RESULT_CANCELED, resultIntent())
        if (!isOurWidget()) {
            finish()
            return
        }
        // Rotating recreates the screen: keep what was picked meanwhile.
        config = savedInstanceState?.getString(STATE_CONFIG)?.let { WidgetConfig.fromJson(it) }
            ?: WidgetConfigStore.get(this, appWidgetId)
        val phoneLook = WidgetTheme.resolve(ThemeChoice.APP, WidgetConfigStore.appTheme(this), WidgetRenderer.phoneNight(this))
        window.decorView.setBackgroundColor(Palette.from(this, phoneLook).background)
        io.execute {
            val loaded = ReplyParser.parseLists(WidgetBridge.call(this, WidgetRequests.lists()))
            runOnUiThread { if (!isDestroyed) show(loaded) }
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString(STATE_CONFIG, config.toJson())
    }

    override fun onDestroy() {
        io.shutdown()
        super.onDestroy()
    }

    /** Only widgets of this provider may be configured here (the activity must be exported for the launcher). */
    private fun isOurWidget(): Boolean {
        if (appWidgetId == AppWidgetManager.INVALID_APPWIDGET_ID) return false
        val info = AppWidgetManager.getInstance(this).getAppWidgetInfo(appWidgetId) ?: return false
        return info.provider.className == TodoWidgetProvider::class.java.name
    }

    private fun resultIntent() = Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)

    // ------------------------------------------------------------------ screen

    private fun show(loaded: ListChoices?) {
        choices = loaded
        // A widget whose List was deleted starts on Today instead of nothing, so Done always means something.
        val listId = config.view.removePrefix("list:").takeIf { config.view.startsWith("list:") }
        if (loaded != null && listId != null && loaded.lists.none { it.id == listId }) {
            config = config.copy(view = WidgetConfig.DEFAULT_VIEW)
        }
        WidgetConfigStore.rememberAppTheme(this, loaded?.appTheme)
        // The setup screen itself wears the app's theme.
        val look = WidgetTheme.resolve(ThemeChoice.APP, loaded?.appTheme ?: WidgetConfigStore.appTheme(this), WidgetRenderer.phoneNight(this))
        palette = Palette.from(this, look)
        window.decorView.setBackgroundColor(palette.background)
        // Let the app's own colour show behind the navigation bar instead of a grey scrim.
        window.isNavigationBarContrastEnforced = false
        WindowInsetsControllerCompat(window, window.decorView).apply {
            isAppearanceLightStatusBars = !WidgetTheme.isDark(look)
            isAppearanceLightNavigationBars = !WidgetTheme.isDark(look)
        }
        // Some launchers/ROMs pick the bar icon colour from the bar's own colour, so set it as well.
        @Suppress("DEPRECATION")
        window.navigationBarColor = palette.background
        @Suppress("DEPRECATION")
        window.statusBarColor = palette.background

        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        root.addView(
            text(getString(R.string.widget_setup_title), 22f, palette.ink, bold = true).apply {
                setPadding(dp(18), dp(20), dp(18), dp(8))
            },
        )

        previewHost = FrameLayout(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                intArrayOf(0xFFF6C79A.toInt(), 0xFF7B6FD6.toInt(), 0xFF3A5A7A.toInt()),
            ).apply { cornerRadius = dp(18).toFloat() }
            clipToOutline = true
            setPadding(dp(10), dp(10), dp(10), dp(10))
        }
        val landscape = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        val previewParams = LinearLayout.LayoutParams(MATCH_PARENT, dp(if (landscape) 170 else 190))
            .apply { setMargins(dp(18), dp(4), dp(18), dp(10)) }

        val options = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(18), 0, dp(18), dp(8))
        }
        buildOptions(options)
        if (landscape) {
            // A sticky preview would take most of a short screen: the whole page scrolls instead.
            val page = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
            page.addView(previewHost, previewParams)
            page.addView(options)
            root.addView(ScrollView(this).apply { addView(page) }, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        } else {
            root.addView(previewHost, previewParams)
            root.addView(ScrollView(this).apply { addView(options) }, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        }
        root.addView(footer())
        setContentView(root, ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        refreshRadios()
        refreshPreview(refetch = true)
    }

    private fun buildOptions(into: LinearLayout) {
        into.addView(heading(getString(R.string.widget_setup_show)))
        fixedViews().forEach { (view, label, hint) -> into.addView(option(label, hint, { config.view == view }) { pick(config.copy(view = view), true) }) }
        choices?.lists?.forEach { list ->
            val view = "list:${list.id}"
            into.addView(option(list.label, getString(R.string.widget_view_list_hint), { config.view == view }) { pick(config.copy(view = view), true) })
        }
        into.addView(heading(getString(R.string.widget_setup_theme)))
        themeChoices().forEach { (choice, label, hint) ->
            into.addView(option(label, hint, { config.theme == choice }) { pick(config.copy(theme = choice), false) })
        }
        into.addView(heading(getString(R.string.widget_setup_background)))
        backgroundChoices().forEach { (background, label) ->
            into.addView(option(label, null, { config.background == background }) { pick(config.copy(background = background), false) })
        }
    }

    private fun fixedViews() = listOf(
        Triple("today", getString(R.string.widget_view_today), getString(R.string.widget_view_today_hint)),
        Triple("upcoming", getString(R.string.widget_view_upcoming), getString(R.string.widget_view_upcoming_hint)),
        Triple("inbox", getString(R.string.widget_view_inbox), getString(R.string.widget_view_inbox_hint)),
        Triple("all", getString(R.string.widget_view_all), getString(R.string.widget_view_all_hint)),
    )

    private fun themeChoices() = listOf(
        Triple(ThemeChoice.APP, getString(R.string.widget_theme_app), getString(R.string.widget_theme_app_hint)),
        Triple(ThemeChoice.LIGHT, getString(R.string.widget_theme_light), null),
        Triple(ThemeChoice.DARK, getString(R.string.widget_theme_dark), null),
        Triple(ThemeChoice.TRUE_BLACK, getString(R.string.widget_theme_true_black), null),
        Triple(ThemeChoice.MATERIAL_YOU, getString(R.string.widget_theme_material_you), null),
        Triple(ThemeChoice.MATERIAL_YOU_DARK, getString(R.string.widget_theme_material_you_dark), null),
    )

    private fun backgroundChoices() = listOf(
        Background.SOLID to getString(R.string.widget_background_solid),
        Background.SEE_THROUGH to getString(R.string.widget_background_see_through),
        Background.NONE to getString(R.string.widget_background_none),
    )

    private fun pick(next: WidgetConfig, viewChanged: Boolean) {
        config = next
        refreshRadios()
        refreshPreview(refetch = viewChanged)
    }

    private fun refreshRadios() = radios.forEach { (radio, selected) -> radio.isChecked = selected() }

    /** Draws the preview with the widget's own renderer; real Tasks when it can read them, samples when not. */
    private fun refreshPreview(refetch: Boolean) {
        val view = config.view
        val seq = ++previewSeq
        io.execute {
            val reply = if (refetch || !replies.containsKey(view)) {
                ReplyParser.parse(WidgetBridge.call(this, WidgetRequests.snapshot(view, System.currentTimeMillis(), ZoneId.systemDefault())))
                    .also { replies[view] = it }
            } else {
                replies.getValue(view)
            }
            runOnUiThread {
                if (isDestroyed || seq != previewSeq) return@runOnUiThread
                val shown = when (reply) {
                    is Reply.Ok, Reply.ListMissing -> reply
                    // Nothing real to show (not set up yet, an error): built-in sample Tasks.
                    else -> Reply.Ok(SampleSnapshot.today(choices?.appTheme))
                }
                drawPreview(shown)
            }
        }
    }

    private fun drawPreview(reply: Reply) {
        previewHost.removeAllViews()
        // Collection items (the list) only inflate inside an AppWidgetHostView, as on the home screen.
        val id = appWidgetId // (inside apply, a bare appWidgetId would be the host view's own, 0)
        val info = AppWidgetManager.getInstance(this).getAppWidgetInfo(id)
        val host = AppWidgetHostView(this).apply {
            // The host needs the provider's info to build its remote context (else it logs an NPE).
            if (info != null) setAppWidget(id, info)
            setPadding(0, 0, 0, 0)
            updateAppWidget(WidgetRenderer.render(this@WidgetSetupActivity, reply, config, previewLines = PREVIEW_LINES))
        }
        previewHost.addView(host, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        // A preview is not interactive: a transparent layer swallows touches.
        previewHost.addView(View(this).apply { isClickable = true }, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }

    private fun footer() = LinearLayout(this).apply {
        gravity = Gravity.END
        setPadding(dp(12), dp(8), dp(18), dp(12))
        addView(
            button(getString(R.string.widget_setup_cancel), filled = false) { finish() },
        )
        addView(
            button(getString(R.string.widget_setup_done), filled = true) { done() },
            LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(8) },
        )
    }

    private fun done() {
        WidgetConfigStore.put(this, appWidgetId, config)
        setResult(RESULT_OK, resultIntent())
        WidgetUpdater.redrawAsync(applicationContext, null)
        finish()
    }

    // ------------------------------------------------------------------ small views

    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()

    private fun text(value: String, sp: Float, color: Int, bold: Boolean = false) = TextView(this).apply {
        text = value
        setTextSize(TypedValue.COMPLEX_UNIT_SP, sp)
        setTextColor(color)
        if (bold) setTypeface(typeface, Typeface.BOLD)
    }

    private fun heading(label: String) = text(label.uppercase(), 11f, palette.faint, bold = true).apply {
        letterSpacing = 0.04f
        setPadding(0, dp(16), 0, dp(2))
    }

    private fun option(label: String, hint: String?, selected: () -> Boolean, onPick: () -> Unit): View {
        val radio = RadioButton(this).apply {
            isClickable = false
            isFocusable = false
            buttonTintList = ColorStateList(
                arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                intArrayOf(palette.todo, palette.faint),
            )
        }
        radios.add(radio to selected)
        val words = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(text(label, 15f, palette.ink))
            if (hint != null) addView(text(hint, 12f, palette.faint))
        }
        return LinearLayout(this).apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(8), 0, dp(8))
            minimumHeight = dp(48)
            isClickable = true
            setOnClickListener { onPick() }
            addView(words, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
            addView(radio)
            contentDescription = if (hint == null) label else "$label, $hint"
        }
    }

    private fun button(label: String, filled: Boolean, onClick: () -> Unit) = Button(this).apply {
        text = label
        isAllCaps = false
        setTextColor(if (filled) palette.onTodo else palette.ink)
        setTypeface(typeface, Typeface.BOLD)
        background = GradientDrawable().apply {
            cornerRadius = dp(10).toFloat()
            setColor(if (filled) palette.todo else 0)
        }
        stateListAnimator = null
        minWidth = dp(72)
        setOnClickListener { onClick() }
    }

    private companion object {
        const val STATE_CONFIG = "config"

        /** Lines the preview shows, so it never crops mid-row. */
        const val PREVIEW_LINES = 4
    }
}
