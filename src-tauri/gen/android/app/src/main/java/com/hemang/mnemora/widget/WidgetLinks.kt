package com.hemang.mnemora.widget

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.hemang.mnemora.MainActivity
import java.net.URLEncoder

/**
 * The links a tap on the Todo widget opens the app with: `mnemora://todo?view=‹view›&task=‹id›`.
 * `view` is the widget's own spelling (`today`, `upcoming`, `inbox`, `all`, `list:‹id›`). The app
 * reads them in `src/modules/todo/deep-link.ts`; `todo-links.json` holds the cases both sides run.
 *
 * The scheme is not declared in the manifest, so no other app can send one. The widget starts the
 * app's own main activity by name with the link as data, so a Production widget can only open
 * Production and a Sandbox widget Sandbox.
 */
object WidgetLinks {
    const val PREFIX = "mnemora://todo"

    /** The link for [view], and for the Task [task] over it when given. */
    fun build(view: String, task: String? = null): String {
        val link = StringBuilder(PREFIX).append("?view=").append(escape(view, keep = ':'))
        if (task != null) link.append("&task=").append(escape(task))
        return link.toString()
    }

    fun isLink(url: String?): Boolean = url != null && (url == PREFIX || url.startsWith("$PREFIX?"))

    /** Percent-encodes [text] for a query value; spaces are %20, and [keep] stays as it is. */
    private fun escape(text: String, keep: Char? = null): String {
        val encoded = URLEncoder.encode(text, "UTF-8").replace("+", "%20")
        return if (keep == null) encoded else encoded.replace("%" + "%02X".format(keep.code), keep.toString())
    }

    /** The intent that opens the app at [url]: explicit to the main activity, ACTION_VIEW with the link as data. */
    fun intent(context: Context, url: String): Intent =
        Intent(context, MainActivity::class.java)
            .setAction(Intent.ACTION_VIEW)
            .setData(Uri.parse(url))
}
