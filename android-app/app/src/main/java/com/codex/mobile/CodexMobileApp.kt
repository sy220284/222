package com.codex.mobile

import android.app.Application

/** Owns the in-process mobile state across Activity recreations and task notifications. */
class CodexMobileApp : Application() {
    val state: MobileState by lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        MobileState(this)
    }

}
