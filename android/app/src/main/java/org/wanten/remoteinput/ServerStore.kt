package org.wanten.remoteinput

import android.content.Context

class ServerStore(context: Context) {
    private val prefs = context.getSharedPreferences("server", Context.MODE_PRIVATE)

    fun get(): ServerUrl? {
        val host = prefs.getString("host", "") ?: ""
        if (host.isEmpty()) return null
        return ServerUrl(
            secure = prefs.getBoolean("secure", true),
            host = host,
            port = if (prefs.contains("port")) prefs.getInt("port", 443) else null,
            fingerprint = prefs.getString("fp", null)
        )
    }

    fun save(server: ServerUrl) {
        val edit = prefs.edit()
        edit.putBoolean("secure", server.secure)
        edit.putString("host", server.host)
        if (server.port != null) edit.putInt("port", server.port)
        else edit.remove("port")
        if (server.fingerprint != null) edit.putString("fp", server.fingerprint)
        else edit.remove("fp")
        edit.apply()
    }
}
