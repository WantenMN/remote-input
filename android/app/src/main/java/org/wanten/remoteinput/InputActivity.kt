package org.wanten.remoteinput

import android.content.Intent
import android.content.res.ColorStateList
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.TextView
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import java.security.cert.CertificateException
import javax.net.ssl.X509TrustManager
import kotlin.concurrent.thread

class InputActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_TEXT = "text"
    }

    private lateinit var serverStore: ServerStore
    private lateinit var history: HistoryStore
    private var server: ServerUrl? = null
    private lateinit var statusView: TextView
    private lateinit var headerIcon: ImageView
    private lateinit var serverLabel: TextView
    private lateinit var sendButton: View
    private lateinit var inputView: EditText
    private var ws: WsClient? = null
    private var connected = false
    private var connecting = false
    private var generation = 0
    private val retryHandler = Handler(Looper.getMainLooper())
    private val retryTask = Runnable { if (!connected) ensureConnected() }

    private val historyLauncher =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode != RESULT_OK) return@registerForActivityResult
            val text = result.data?.getStringExtra(EXTRA_TEXT) ?: return@registerForActivityResult
            val current = inputView.text.toString()
            inputView.setText(if (current.isEmpty()) text else "$current\n$text")
            inputView.setSelection(inputView.text.length)
        }

    private val scanLauncher =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            if (result.resultCode != RESULT_OK) return@registerForActivityResult
            val raw = result.data?.getStringExtra(ScanActivity.EXTRA_URL)
            val parsed = raw?.let { parseServerUrl(it) }
            if (parsed == null) {
                toast("Not a valid QR code")
                return@registerForActivityResult
            }
            server = parsed
            updateServerLabel()
            ensureConnected()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_input)
        applyContentInsets()
        serverStore = ServerStore(this)
        history = HistoryStore(this)
        statusView = findViewById(R.id.status_text)
        headerIcon = findViewById(R.id.header_icon)
        serverLabel = findViewById(R.id.server_label)
        inputView = findViewById(R.id.input_text)
        sendButton = findViewById<View>(R.id.send_button).apply { pressScale() }
        sendButton.setOnClickListener { send() }
        findViewById<ImageButton>(R.id.clear_button).apply { pressScale() }.setOnClickListener {
            inputView.setText("")
        }
        findViewById<ImageButton>(R.id.history_button).apply { pressScale() }.setOnClickListener {
            historyLauncher.launch(Intent(this, HistoryActivity::class.java))
        }
        findViewById<ImageButton>(R.id.scan_button).apply { pressScale() }.setOnClickListener {
            scanLauncher.launch(Intent(this, ScanActivity::class.java))
        }
        server = serverStore.get()
        updateServerLabel()
        ensureConnected()
    }

    override fun onResume() {
        super.onResume()
        if (!connected && !connecting) ensureConnected()
    }

    override fun onPause() {
        super.onPause()
        retryHandler.removeCallbacks(retryTask)
    }

    override fun onDestroy() {
        super.onDestroy()
        retryHandler.removeCallbacks(retryTask)
        if (::statusView.isInitialized) statusView.removeCallbacks(clearTask)
        closeWs()
    }

    private fun setStatus(text: String) {
        runOnUiThread {
            statusView.text = text
            statusView.removeCallbacks(clearTask)
            statusView.postDelayed(clearTask, 2500)
        }
    }

    private val clearTask = Runnable { statusView.text = "" }

    private fun showState(colorRes: Int) {
        runOnUiThread {
            val color = getColor(colorRes)
            headerIcon.imageTintList = ColorStateList.valueOf(color)
            sendButton.backgroundTintList = ColorStateList.valueOf(color)
            inputView.setBackgroundResource(
                when (colorRes) {
                    R.color.green -> R.drawable.bg_input_ok
                    R.color.amber -> R.drawable.bg_input_warn
                    else -> R.drawable.bg_input_err
                }
            )
        }
    }

    private fun updateServerLabel() {
        serverLabel.text = server?.label() ?: ""
    }

    private fun closeWs() {
        ws?.shutdown()
        ws = null
    }

    private fun scheduleRetry() {
        retryHandler.removeCallbacks(retryTask)
        retryHandler.postDelayed(retryTask, 3000)
    }

    private fun ensureConnected() {
        val current = server
        if (current == null) {
            showState(R.color.red)
            setStatus("Scan a QR code to connect")
            return
        }
        generation++
        connected = false
        connecting = true
        retryHandler.removeCallbacks(retryTask)
        showState(R.color.amber)
        closeWs()
        val gen = generation
        if (!current.secure) {
            connect(null, gen)
            return
        }
        val pinned = current.fingerprint
        if (pinned != null) {
            connect(pinTrustManager(pinned), gen)
            return
        }
        thread {
            try {
                val fingerprint = fetchServerFingerprint(current.host, current.effectivePort(), 2000)
                if (gen != generation) return@thread
                current.fingerprint = fingerprint
                runOnUiThread { if (gen == generation) connect(pinTrustManager(fingerprint), gen) }
            } catch (e: Exception) {
                if (gen != generation) return@thread
                connected = false
                connecting = false
                showState(R.color.red)
                setStatus("Connection failed: ${e.message ?: "unknown error"}")
                scheduleRetry()
            }
        }
    }

    private fun connect(trustManager: X509TrustManager?, gen: Int) {
        val current = server ?: return
        val client = WsClient(current, trustManager)
        ws = client
        client.connect(object : WsClient.Listener {
            override fun onOpen() {
                if (gen != generation) return
                connected = true
                connecting = false
                retryHandler.removeCallbacks(retryTask)
                server?.let { serverStore.save(it) }
                showState(R.color.green)
            }

            override fun onTextAck(chars: Int) {
                runOnUiThread { sendButton.pop() }
                setStatus("Sent $chars chars")
            }

            override fun onClosed(code: Int, reason: String) {
                if (gen != generation) return
                connected = false
                connecting = false
                showState(R.color.red)
                setStatus(closeText(code, reason))
                scheduleRetry()
            }

            override fun onFailure(error: Throwable) {
                if (gen != generation) return
                if (isPinMismatch(error)) {
                    refreshPin(gen)
                    return
                }
                connected = false
                connecting = false
                showState(R.color.red)
                setStatus("Connection failed: ${error.message ?: "unknown error"}")
                scheduleRetry()
            }
        })
    }

    private fun refreshPin(gen: Int) {
        thread {
            try {
                val current = server ?: return@thread
                val fingerprint = fetchServerFingerprint(current.host, current.effectivePort(), 2000)
                if (gen != generation) return@thread
                current.fingerprint = fingerprint
                runOnUiThread { if (gen == generation) connect(pinTrustManager(fingerprint), gen) }
            } catch (e: Exception) {
                if (gen != generation) return@thread
                connected = false
                connecting = false
                showState(R.color.red)
                setStatus("Connection failed: ${e.message ?: "unknown error"}")
                scheduleRetry()
            }
        }
    }

    private fun closeText(code: Int, reason: String): String {
        if (code == 1013) {
            if (reason.contains("whitelist", ignoreCase = true)) return "IP not in whitelist"
            if (reason.contains("capacity", ignoreCase = true)) return "Desktop is busy with another device"
        }
        return if (reason.isNotEmpty()) "Connection closed: $reason" else "Connection closed ($code)"
    }

    private fun isPinMismatch(error: Throwable?): Boolean {
        var cause = error
        while (cause != null) {
            if (cause is CertificateException && cause.message == "pin mismatch") return true
            cause = cause.cause
        }
        return false
    }

    private fun send() {
        val text = inputView.text.toString().trim()
        if (text.isEmpty()) return
        val client = ws
        if (client == null || !client.send(text)) {
            toast("Not connected")
            return
        }
        if (history.recording) history.add(text)
        inputView.setText("")
    }
}
