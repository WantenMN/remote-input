package org.wanten.remoteinput

import android.app.Activity
import android.content.Context
import android.view.MotionEvent
import android.view.View
import android.widget.Toast
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

fun Context.toast(msg: String) {
    Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
}

fun Activity.applyContentInsets() {
    WindowCompat.setDecorFitsSystemWindows(window, false)
    val root = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
        v.setPadding(v.paddingLeft, bars.top, v.paddingRight, maxOf(bars.bottom, ime.bottom))
        insets
    }
}

fun View.pressScale() {
    setOnTouchListener { v, event ->
        when (event.action) {
            MotionEvent.ACTION_DOWN -> v.animate().scaleX(0.96f).scaleY(0.96f).setDuration(80).start()
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL ->
                v.animate().scaleX(1f).scaleY(1f).setDuration(120).start()
        }
        false
    }
}

fun View.pop() {
    animate().scaleX(0.92f).scaleY(0.92f).setDuration(100).withEndAction {
        animate().scaleX(1.05f).scaleY(1.05f).setDuration(120).withEndAction {
            animate().scaleX(1f).scaleY(1f).setDuration(120).start()
        }.start()
    }.start()
}
