package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetConfigTest {
    @Test
    fun theDefaultsAreTodaySameAsAppSolid() {
        val config = WidgetConfig()
        assertEquals("today", config.view)
        assertEquals(ThemeChoice.APP, config.theme)
        assertEquals(Background.SOLID, config.background)
    }

    @Test
    fun aConfigRoundTripsThroughJson() {
        val config = WidgetConfig("list:0199-abc", ThemeChoice.DARK, Background.SEE_THROUGH)
        assertEquals(config, WidgetConfig.fromJson(config.toJson()))
        for (view in listOf("today", "upcoming", "inbox", "all")) {
            val c = WidgetConfig(view, ThemeChoice.TRUE_BLACK, Background.NONE)
            assertEquals(c, WidgetConfig.fromJson(c.toJson()))
        }
    }

    @Test
    fun aWidgetSavedWithARetiredMaterialYouThemeFollowsTheApp() {
        val json = "{\"view\":\"today\",\"theme\":\"you_dark\",\"background\":\"solid\"}"
        assertEquals(ThemeChoice.APP, WidgetConfig.fromJson(json).theme)
    }

    @Test
    fun theJsonUsesStableKeys() {
        val json = org.json.JSONObject(WidgetConfig("inbox", ThemeChoice.TRUE_BLACK, Background.SEE_THROUGH).toJson())
        assertEquals("inbox", json.getString("view"))
        assertEquals("oled", json.getString("theme"))
        assertEquals("see_through", json.getString("background"))
    }

    @Test
    fun missingOrBrokenJsonIsTheDefault() {
        for (bad in listOf(null, "", "not json", "[]", "{}")) {
            assertEquals("$bad", WidgetConfig(), WidgetConfig.fromJson(bad))
        }
    }

    @Test
    fun unknownValuesFallBackOneByOne() {
        val config = WidgetConfig.fromJson("""{"view":"narnia","theme":"oled","background":"glass"}""")
        assertEquals(WidgetConfig("today", ThemeChoice.TRUE_BLACK, Background.SOLID), config)
        assertEquals("today", WidgetConfig.fromJson("""{"view":"list:"}""").view)
    }

    @Test
    fun onlyKnownViewsAreViews() {
        assertTrue(WidgetConfig.isView("upcoming"))
        assertTrue(WidgetConfig.isView("list:42"))
        assertFalse(WidgetConfig.isView("list:"))
        assertFalse(WidgetConfig.isView("list"))
        assertFalse(WidgetConfig.isView(null))
        assertFalse(WidgetConfig.isView(""))
    }
}
