package com.hemang.mnemora.widget

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetThemeTest {
    @Test
    fun sameAsAppFollowsTheAppsTheme() {
        assertEquals(Look.LIGHT, WidgetTheme.resolve(ThemeChoice.APP, "light", phoneNight = true))
        assertEquals(Look.DARK, WidgetTheme.resolve(ThemeChoice.APP, "dark", phoneNight = false))
        assertEquals(Look.TRUE_BLACK, WidgetTheme.resolve(ThemeChoice.APP, "oled", phoneNight = false))
    }

    @Test
    fun sameAsAppOnSystemFollowsThePhonesNightMode() {
        assertEquals(Look.LIGHT, WidgetTheme.resolve(ThemeChoice.APP, "system", phoneNight = false))
        assertEquals(Look.DARK, WidgetTheme.resolve(ThemeChoice.APP, "system", phoneNight = true))
    }

    @Test
    fun anUnknownAppThemeFollowsThePhoneToo() {
        // The app has not run yet, or the reply carried no theme.
        assertEquals(Look.DARK, WidgetTheme.resolve(ThemeChoice.APP, null, phoneNight = true))
        assertEquals(Look.LIGHT, WidgetTheme.resolve(ThemeChoice.APP, "neon", phoneNight = false))
    }

    @Test
    fun anExplicitChoiceIgnoresTheAppAndThePhone() {
        for (appTheme in listOf("light", "dark", "oled", "system", null)) {
            for (night in listOf(true, false)) {
                assertEquals(Look.LIGHT, WidgetTheme.resolve(ThemeChoice.LIGHT, appTheme, night))
                assertEquals(Look.DARK, WidgetTheme.resolve(ThemeChoice.DARK, appTheme, night))
                assertEquals(Look.TRUE_BLACK, WidgetTheme.resolve(ThemeChoice.TRUE_BLACK, appTheme, night))
            }
        }
    }

    @Test
    fun darkLooksAreTheDarkOnes() {
        assertFalse(WidgetTheme.isDark(Look.LIGHT))
        assertTrue(WidgetTheme.isDark(Look.DARK))
        assertTrue(WidgetTheme.isDark(Look.TRUE_BLACK))
    }

    @Test
    fun backgroundsKeepTheirAlpha() {
        assertEquals(1f, Background.SOLID.alpha)
        assertEquals(0.72f, Background.SEE_THROUGH.alpha)
        assertEquals(0f, Background.NONE.alpha)
    }

    @Test
    fun everyChoiceRoundTripsThroughItsKey() {
        for (choice in ThemeChoice.values()) assertEquals(choice, ThemeChoice.fromKey(choice.key))
        for (background in Background.values()) assertEquals(background, Background.fromKey(background.key))
        assertEquals(ThemeChoice.APP, ThemeChoice.fromKey("neon"))
        // Material You was removed: widgets saved with its keys follow the app's theme.
        assertEquals(ThemeChoice.APP, ThemeChoice.fromKey("you"))
        assertEquals(ThemeChoice.APP, ThemeChoice.fromKey("you_dark"))
        assertEquals(Background.SOLID, Background.fromKey(null))
    }
}

class TextEdgeTest {
    @Test
    fun aCardNeedsNoEdgeOnTheText() {
        for (look in Look.values()) {
            assertEquals(TextEdge.NONE, TextEdge.of(Background.SOLID, look))
            assertEquals(TextEdge.NONE, TextEdge.of(Background.SEE_THROUGH, look))
        }
    }

    @Test
    fun withNoBackgroundDarkThemesGetAShadowAndLightOnesAGlow() {
        assertEquals(TextEdge.GLOW, TextEdge.of(Background.NONE, Look.LIGHT))
        assertEquals(TextEdge.SHADOW, TextEdge.of(Background.NONE, Look.DARK))
        assertEquals(TextEdge.SHADOW, TextEdge.of(Background.NONE, Look.TRUE_BLACK))
    }

    @Test
    fun onlySameAsAppOnSystemFollowsThePhoneLive() {
        assertTrue(WidgetTheme.followsPhone(ThemeChoice.APP, "system"))
        assertTrue(WidgetTheme.followsPhone(ThemeChoice.APP, null))
        assertFalse(WidgetTheme.followsPhone(ThemeChoice.APP, "dark"))
        assertFalse(WidgetTheme.followsPhone(ThemeChoice.APP, "light"))
        assertFalse(WidgetTheme.followsPhone(ThemeChoice.APP, "oled"))
        assertFalse(WidgetTheme.followsPhone(ThemeChoice.DARK, "system"))
    }
}
