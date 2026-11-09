package org.wanten.remoteinput

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.PorterDuffXfermode
import android.graphics.RectF
import android.util.AttributeSet
import android.view.View

class ScanOverlayView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null
) : View(context, attrs) {
    private val density = resources.displayMetrics.density
    private var frame = RectF()
    private var dim: Bitmap? = null
    private var laser = 0f
    private var active = true
    private val animator = ValueAnimator.ofFloat(0f, 1f).apply {
        duration = 2200
        repeatCount = ValueAnimator.INFINITE
        repeatMode = ValueAnimator.REVERSE
        addUpdateListener {
            laser = animatedValue as Float
            invalidate()
        }
    }
    private val dimPaint = Paint().apply { color = 0xA8000000.toInt() }
    private val clearPaint = Paint().apply {
        xfermode = PorterDuffXfermode(PorterDuff.Mode.CLEAR)
    }
    private val cornerPaint = Paint().apply {
        color = Color.WHITE
        style = Paint.Style.STROKE
        strokeWidth = 5f * density
        strokeCap = Paint.Cap.ROUND
        isAntiAlias = true
    }
    private val laserGlowPaint = Paint().apply {
        color = 0x334ADE80.toInt()
        isAntiAlias = true
    }
    private val laserMidPaint = Paint().apply {
        color = 0x994ADE80.toInt()
        isAntiAlias = true
    }
    private val laserPaint = Paint().apply {
        color = 0xFF4ADE80.toInt()
        isAntiAlias = true
    }
    private val hintPaint = Paint().apply {
        color = Color.WHITE
        textSize = 16f * density
        textAlign = Paint.Align.CENTER
        isAntiAlias = true
        setShadowLayer(4f * density, 0f, 2f * density, 0x99000000.toInt())
    }

    fun setActive(on: Boolean) {
        active = on
        if (on && !animator.isRunning) animator.start()
        if (!on) animator.cancel()
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        if (active) animator.start()
    }

    override fun onDetachedFromWindow() {
        animator.cancel()
        dim?.recycle()
        dim = null
        super.onDetachedFromWindow()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        if (w == 0 || h == 0) return
        val side = minOf(w, h) * 0.68f
        val left = (w - side) / 2f
        val top = (h - side) / 2f - h * 0.05f
        frame = RectF(left, top, left + side, top + side)
        dim?.recycle()
        val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), dimPaint)
        canvas.drawRoundRect(frame, 24f * density, 24f * density, clearPaint)
        val r = 24f * density
        val pts = floatArrayOf(
            frame.left + r, frame.top, frame.left, frame.top, frame.left, frame.top + r,
            frame.right - r, frame.top, frame.right, frame.top, frame.right, frame.top + r,
            frame.left + r, frame.bottom, frame.left, frame.bottom, frame.left, frame.bottom - r,
            frame.right - r, frame.bottom, frame.right, frame.bottom, frame.right, frame.bottom - r
        )
        for (i in pts.indices step 6) {
            canvas.drawLine(pts[i], pts[i + 1], pts[i + 2], pts[i + 3], cornerPaint)
            canvas.drawLine(pts[i + 2], pts[i + 3], pts[i + 4], pts[i + 5], cornerPaint)
        }
        dim = bmp
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        dim?.let { canvas.drawBitmap(it, 0f, 0f, null) }
        if (frame.isEmpty) return
        if (!active) {
            canvas.drawText(
                context.getString(R.string.rescan),
                frame.centerX(),
                frame.centerY(),
                hintPaint
            )
            return
        }
        val y = frame.top + laser * frame.height()
        val inset = frame.width() * 0.06f
        canvas.drawRoundRect(
            frame.left + inset, y - 9f * density,
            frame.right - inset, y + 9f * density,
            9f * density, 9f * density, laserGlowPaint
        )
        canvas.drawRoundRect(
            frame.left + inset, y - 4f * density,
            frame.right - inset, y + 4f * density,
            4f * density, 4f * density, laserMidPaint
        )
        canvas.drawRoundRect(
            frame.left + inset, y - 1.5f * density,
            frame.right - inset, y + 1.5f * density,
            2f * density, 2f * density, laserPaint
        )
    }
}
