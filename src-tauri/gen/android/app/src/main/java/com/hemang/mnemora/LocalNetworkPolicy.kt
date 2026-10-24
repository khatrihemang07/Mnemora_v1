package com.hemang.mnemora

/**
 * Android 17's local network permission (`ACCESS_LOCAL_NETWORK`) for Sync: when it exists, and
 * whether asking again can still show the system's dialog. Pure, so it can be tested without a
 * device. Sync asks only when it is turned on, never while it is off.
 */
object LocalNetworkPolicy {
    const val PERMISSION = "android.permission.ACCESS_LOCAL_NETWORK"
    /** The first Android version that has the permission. */
    const val FIRST_SDK = 37

    const val UNNEEDED = "unneeded"
    const val GRANTED = "granted"
    const val DENIED = "denied"
    /** Not answered yet (never asked, or the dialog is up): not a refusal, so nothing is blamed. */
    const val ASKING = "asking"

    /** Whether the permission exists on this Android version. */
    fun applies(sdk: Int): Boolean = sdk >= FIRST_SDK

    /** What the web side is told: [UNNEEDED] where the permission does not exist. */
    fun state(sdk: Int, granted: Boolean, answered: Boolean = true): String = when {
        !applies(sdk) -> UNNEEDED
        granted -> GRANTED
        !answered -> ASKING
        else -> DENIED
    }

    /**
     * Whether the system's dialog can still be shown: the first time, and again while the system
     * says a rationale is due. After "Don't ask again" only the app's settings page can grant it.
     */
    fun canAsk(askedBefore: Boolean, rationaleDue: Boolean): Boolean = !askedBefore || rationaleDue
}
