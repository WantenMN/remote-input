package org.wanten.remoteinput

data class ServerUrl(
    val secure: Boolean,
    val host: String,
    val port: Int?,
    var fingerprint: String?
) {
    val wsUrl: String
        get() = (if (secure) "wss" else "ws") + "://$host:${effectivePort()}/ws"

    fun label(): String {
        val base = (if (secure) "https" else "http") + "://$host"
        return if (port != null) "$base:$port" else base
    }
}

fun ServerUrl.effectivePort(): Int = port ?: if (secure) 443 else 80

fun parseServerUrl(text: String): ServerUrl? {
    val raw = text.trim()
    if (raw.isEmpty()) return null
    val secure: Boolean
    val rest: String
    when {
        raw.startsWith("https://", ignoreCase = true) -> {
            secure = true
            rest = raw.substring(8)
        }
        raw.startsWith("http://", ignoreCase = true) -> {
            secure = false
            rest = raw.substring(7)
        }
        else -> return null
    }
    val hashIdx = rest.indexOf('#')
    val authority = if (hashIdx >= 0) rest.substring(0, hashIdx) else rest
    val fragment = if (hashIdx >= 0) rest.substring(hashIdx + 1) else ""
    if (authority.isEmpty() || authority.contains('/')) return null
    val host: String
    val port: Int?
    if (authority.startsWith("[")) {
        val close = authority.indexOf(']')
        if (close < 0) return null
        host = authority.substring(1, close)
        val tail = authority.substring(close + 1)
        port = if (tail.isEmpty()) null else {
            if (!tail.startsWith(":")) return null
            tail.substring(1).toIntOrNull() ?: return null
        }
    } else {
        val colon = authority.lastIndexOf(':')
        if (colon < 0) {
            host = authority
            port = null
        } else {
            host = authority.substring(0, colon)
            port = authority.substring(colon + 1).toIntOrNull() ?: return null
        }
    }
    if (host.isEmpty() || (port != null && port !in 1..65535)) return null
    var fingerprint: String? = null
    if (fragment.startsWith("fp=")) {
        var hex = fragment.substring(3)
        if (hex.startsWith("SHA256:", ignoreCase = true)) hex = hex.substring(7)
        hex = hex.uppercase()
        if (hex.length != 64 || !hex.all { it in '0'..'9' || it in 'A'..'F' }) return null
        fingerprint = hex
    }
    return ServerUrl(secure, host, port, fingerprint)
}
