package org.wanten.remoteinput

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.media.Image
import android.os.Bundle
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import com.google.zxing.BinaryBitmap
import com.google.zxing.ReaderException
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.qrcode.QRCodeReader
import java.security.SecureRandom
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import kotlin.concurrent.thread
import okhttp3.OkHttpClient
import okhttp3.Request

class ScanActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_URL = "url"
    }

    private val permissionLauncher =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) startCamera() else {
                toast("Camera permission is required to scan")
                finish()
            }
        }

    private var cameraProvider: ProcessCameraProvider? = null
    private var cameraBound = false
    private lateinit var cameraExecutor: ExecutorService
    private lateinit var manual: EditText
    private lateinit var preview: PreviewView
    private lateinit var overlay: ScanOverlayView
    private val done = AtomicBoolean(false)
    private val reader = QRCodeReader()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_scan)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val bottomBar = findViewById<View>(R.id.manual_bar)
        val baseBottom = bottomBar.paddingBottom
        ViewCompat.setOnApplyWindowInsetsListener(bottomBar) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            v.setPadding(v.paddingLeft, v.paddingTop, v.paddingRight, baseBottom + maxOf(bars.bottom, ime.bottom))
            insets
        }
        manual = findViewById(R.id.manual_input)
        preview = findViewById(R.id.preview)
        overlay = findViewById(R.id.overlay)
        cameraExecutor = Executors.newSingleThreadExecutor()
        manual.setText("https://")
        manual.setSelection(manual.text.length)
        manual.setOnFocusChangeListener { _, focused ->
            if (focused) stopCamera()
        }
        findViewById<Button>(R.id.manual_go).apply { pressScale() }.setOnClickListener {
            submit(manual.text.toString())
        }
        preview.setOnClickListener { restartPreview() }
        overlay.setOnClickListener { restartPreview() }
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            startCamera()
        } else {
            permissionLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        stopCamera()
        cameraExecutor.shutdown()
    }

    private fun hideKeyboard() {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(manual.windowToken, 0)
    }

    private fun startCamera() {
        preview.visibility = View.VISIBLE
        overlay.setActive(true)
        val future = ProcessCameraProvider.getInstance(this)
        future.addListener(
            {
                val provider = future.get()
                cameraProvider = provider
                val useCase = Preview.Builder().build()
                useCase.setSurfaceProvider(preview.surfaceProvider)
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(cameraExecutor) { proxy -> analyze(proxy) }
                provider.unbindAll()
                provider.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, useCase, analysis)
                cameraBound = true
            },
            ContextCompat.getMainExecutor(this)
        )
    }

    private fun stopCamera() {
        cameraBound = false
        try {
            cameraProvider?.unbindAll()
        } catch (e: Exception) {
        }
        preview.visibility = View.INVISIBLE
        overlay.setActive(false)
    }

    private fun restartPreview() {
        manual.clearFocus()
        hideKeyboard()
        if (!cameraBound) startCamera()
    }

    private fun analyze(proxy: ImageProxy) {
        try {
            val image = proxy.image ?: return
            val text = decodeQr(image) ?: return
            runOnUiThread { submit(text) }
        } finally {
            proxy.close()
        }
    }

    private enum class Probe {
        OK,
        UNREACHABLE,
        WRONG_SERVICE
    }

    private fun submit(text: String) {
        if (!done.compareAndSet(false, true)) return
        val parsed = parseServerUrl(text.trim())
        if (parsed == null) {
            done.set(false)
            toast("Not a valid address")
            return
        }
        thread {
            val result = probeServer(parsed)
            runOnUiThread {
                if (result != Probe.OK) {
                    done.set(false)
                    toast(
                        if (result == Probe.WRONG_SERVICE) "Not a Remote Input server"
                        else "Cannot connect to this address"
                    )
                    return@runOnUiThread
                }
                setResult(RESULT_OK, Intent().putExtra(EXTRA_URL, text.trim()))
                finish()
            }
        }
    }

    private fun probeServer(server: ServerUrl): Probe {
        return try {
            val builder = OkHttpClient.Builder()
                .connectTimeout(2, TimeUnit.SECONDS)
                .readTimeout(2, TimeUnit.SECONDS)
            val scheme: String
            if (server.secure) {
                scheme = "https"
                val trustAll = trustAllManager()
                val ctx = SSLContext.getInstance("TLS")
                ctx.init(null, arrayOf<TrustManager>(trustAll), SecureRandom())
                builder.sslSocketFactory(ctx.socketFactory, trustAll)
                builder.hostnameVerifier { _, _ -> true }
            } else {
                scheme = "http"
            }
            val request = Request.Builder()
                .url("$scheme://${server.host}:${server.effectivePort()}/")
                .build()
            builder.build().newCall(request).execute().use { response ->
                val body = response.body?.string() ?: ""
                if (response.isSuccessful &&
                    body.contains("<title>Remote Input</title>") &&
                    body.contains("<div id=\"root\"")
                ) {
                    Probe.OK
                } else {
                    Probe.WRONG_SERVICE
                }
            }
        } catch (e: Exception) {
            Probe.UNREACHABLE
        }
    }

    private fun decodeQr(image: Image): String? {
        val plane = image.planes[0]
        val width = image.width
        val height = image.height
        val rowStride = plane.rowStride
        val buffer = plane.buffer
        val data = ByteArray(width * height)
        if (rowStride == width) {
            buffer.get(data)
        } else {
            val row = ByteArray(rowStride)
            var pos = 0
            for (r in 0 until height) {
                buffer.get(row)
                System.arraycopy(row, 0, data, pos, width)
                pos += width
            }
        }
        val source = PlanarYUVLuminanceSource(data, width, height, 0, 0, width, height, false)
        return try {
            reader.decode(BinaryBitmap(HybridBinarizer(source))).text
        } catch (e: ReaderException) {
            null
        }
    }
}
