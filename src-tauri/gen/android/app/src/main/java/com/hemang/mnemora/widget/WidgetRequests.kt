package com.hemang.mnemora.widget

import org.json.JSONObject
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/** Builds the bridge's JSON requests. The device clock goes with every one: Rust has no time zone. */
object WidgetRequests {
    private val DATE = DateTimeFormatter.ofPattern("yyyy-MM-dd")
    private val TIME = DateTimeFormatter.ofPattern("HH:mm")

    /** `{today, now, offset_min}` for the instant, on the zone's calendar. */
    fun local(epochMs: Long, zone: ZoneId): JSONObject {
        val at = Instant.ofEpochMilli(epochMs).atZone(zone)
        return JSONObject()
            .put("today", at.format(DATE))
            .put("now", at.format(TIME))
            .put("offset_min", at.offset.totalSeconds / 60)
    }

    /** A view as the widget draws it. */
    fun snapshot(view: String, epochMs: Long, zone: ZoneId): String =
        JSONObject()
            .put("method", "snapshot")
            .put("view", view)
            .put("local", local(epochMs, zone))
            .toString()

    /** The Lists for the setup screen, with the app's theme. */
    fun lists(): String = JSONObject().put("method", "lists").toString()
}
