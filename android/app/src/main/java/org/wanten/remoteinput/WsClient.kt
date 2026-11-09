package org.wanten.remoteinput

import java.security.SecureRandom
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject

class WsClient(
    private val server: ServerUrl,
    private val trustManager: X509TrustManager?
) {
    interface Listener {
        fun onOpen()
        fun onTextAck(chars: Int)
        fun onClosed(code: Int, reason: String)
        fun onFailure(error: Throwable)
    }

    private var client: OkHttpClient? = null
    private var socket: WebSocket? = null
    @Volatile private var open = false

    fun connect(listener: Listener) {
        val builder = OkHttpClient.Builder()
        val tm = trustManager
        if (tm != null) {
            val ctx = SSLContext.getInstance("TLS")
            ctx.init(null, arrayOf<TrustManager>(tm), SecureRandom())
            builder.sslSocketFactory(ctx.socketFactory, tm)
        }
        client = builder.build()
        val request = Request.Builder().url(server.wsUrl).build()
        socket = client!!.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                open = true
                listener.onOpen()
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                try {
                    val obj = JSONObject(text)
                    if (obj.optBoolean("ok", false)) listener.onTextAck(obj.optInt("len", 0))
                } catch (e: Exception) {
                }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                open = false
                listener.onClosed(code, reason)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                open = false
                listener.onFailure(t)
            }
        })
    }

    fun send(text: String): Boolean {
        val trimmed = text.trim()
        if (trimmed.isEmpty() || !open) return false
        return socket?.send(trimmed) == true
    }

    fun shutdown() {
        try {
            socket?.close(1000, null)
        } catch (e: Exception) {
        }
        client?.dispatcher?.executorService?.shutdown()
        client?.connectionPool?.evictAll()
    }
}
