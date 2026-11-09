package org.wanten.remoteinput

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

data class HistoryItem(
    val id: String,
    val text: String,
    val pinned: Boolean,
    val time: Long
)

class HistoryStore(context: Context) {
    private val prefs = context.getSharedPreferences("history", Context.MODE_PRIVATE)

    var recording: Boolean
        get() = prefs.getBoolean("recording", true)
        set(value) {
            prefs.edit().putBoolean("recording", value).apply()
        }

    fun items(): List<HistoryItem> {
        val raw = prefs.getString("items", "[]") ?: "[]"
        val out = mutableListOf<HistoryItem>()
        val arr = JSONArray(raw)
        for (i in 0 until arr.length()) {
            val obj = arr.getJSONObject(i)
            out.add(
                HistoryItem(
                    id = obj.getString("id"),
                    text = obj.getString("text"),
                    pinned = obj.optBoolean("pinned", false),
                    time = obj.optLong("time", 0)
                )
            )
        }
        return out.sortedWith(compareByDescending<HistoryItem> { it.pinned }.thenByDescending { it.time })
    }

    fun add(text: String) {
        val item = HistoryItem(UUID.randomUUID().toString(), text, false, System.currentTimeMillis())
        val kept = (listOf(item) + items()).take(200)
        save(kept)
    }

    fun setPinned(id: String, pinned: Boolean) {
        save(items().map { if (it.id == id) it.copy(pinned = pinned) else it })
    }

    fun delete(id: String) {
        save(items().filter { it.id != id })
    }

    fun clearUnpinned() {
        save(items().filter { it.pinned })
    }

    fun clearAll() {
        save(emptyList())
    }

    private fun save(items: List<HistoryItem>) {
        val arr = JSONArray()
        for (item in items) {
            arr.put(
                JSONObject()
                    .put("id", item.id)
                    .put("text", item.text)
                    .put("pinned", item.pinned)
                    .put("time", item.time)
            )
        }
        prefs.edit().putString("items", arr.toString()).apply()
    }
}
