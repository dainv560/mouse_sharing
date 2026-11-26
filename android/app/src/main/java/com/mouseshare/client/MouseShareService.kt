package com.mouseshare.client

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Color
import android.graphics.Path
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.widget.ImageView

class MouseShareService : AccessibilityService() {

    companion object {
        var instance: MouseShareService? = null
    }

    private var networkClient: NetworkClient? = null
    private var windowManager: WindowManager? = null
    private var cursorView: View? = null
    private var cursorParams: WindowManager.LayoutParams? = null

    private var cursorX = 500f
    private var cursorY = 800f
    private val mainHandler = Handler(Looper.getMainLooper())

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        initCursorOverlay()
    }

    override fun onDestroy() {
        super.onDestroy()
        disconnect()
        removeCursorOverlay()
        instance = null
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}
    override fun onInterrupt() {}

    private fun initCursorOverlay() {
        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager

        val cursor = ImageView(this).apply {
            setImageResource(android.R.drawable.arrow_up_float)
            setColorFilter(Color.WHITE)
            rotation = -45f
        }

        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }

        cursorParams = WindowManager.LayoutParams(
            64,
            64,
            type,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                    WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                    WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = cursorX.toInt()
            y = cursorY.toInt()
        }

        cursorView = cursor
        cursorView?.visibility = View.GONE
        windowManager?.addView(cursorView, cursorParams)
    }

    private fun removeCursorOverlay() {
        cursorView?.let {
            windowManager?.removeView(it)
            cursorView = null
        }
    }

    fun isConnected(): Boolean = networkClient?.isConnected == true

    fun connect(ip: String, port: Int, callback: (Boolean, String) -> Unit) {
        val dm: DisplayMetrics = resources.displayMetrics
        val screenWidth = dm.widthPixels
        val screenHeight = dm.heightPixels

        networkClient = NetworkClient(ip, port, screenWidth, screenHeight, object : NetworkClient.EventListener {
            override fun onConnected() {
                mainHandler.post {
                    cursorView?.visibility = View.VISIBLE
                    callback(true, "Kết nối thành công")
                }
            }

            override fun onDisconnected(reason: String) {
                mainHandler.post {
                    cursorView?.visibility = View.GONE
                    callback(false, reason)
                }
            }

            override fun onMouseMove(x: Int, y: Int) {
                cursorX = x.toFloat()
                cursorY = y.toFloat()
                mainHandler.post {
                    cursorParams?.let {
                        it.x = cursorX.toInt()
                        it.y = cursorY.toInt()
                        windowManager?.updateViewLayout(cursorView, it)
                    }
                }
            }

            override fun onMouseButton(button: Int, isDown: Boolean) {
                if (button == 1 && !isDown) {
                    // Tap on release
                    tapAt(cursorX, cursorY)
                } else if (button == 2 && !isDown) {
                    // Right click -> Back
                    performGlobalAction(GLOBAL_ACTION_BACK)
                } else if (button == 3 && !isDown) {
                    // Middle click -> Home
                    performGlobalAction(GLOBAL_ACTION_HOME)
                }
            }

            override fun onMouseWheel(delta: Int) {
                val endY = if (delta > 0) cursorY + 400 else cursorY - 400
                swipe(cursorX, cursorY, cursorX, endY)
            }

            override fun onKey(vk: Int, isDown: Boolean) {
                if (!isDown) return
                when (vk) {
                    0x1B -> performGlobalAction(GLOBAL_ACTION_BACK)      // Escape
                    0x5B -> performGlobalAction(GLOBAL_ACTION_HOME)      // Windows key -> Home
                    0x09 -> performGlobalAction(GLOBAL_ACTION_RECENTS)   // Tab / Alt-Tab -> Recents
                }
            }

            override fun onClipboardText(text: String) {
                mainHandler.post {
                    val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    val clip = ClipData.newPlainText("MouseShare", text)
                    clipboard.setPrimaryClip(clip)
                }
            }
        })

        networkClient?.start()
    }

    fun disconnect() {
        networkClient?.stop()
        networkClient = null
        mainHandler.post {
            cursorView?.visibility = View.GONE
        }
    }

    private fun tapAt(x: Float, y: Float) {
        val path = Path().apply {
            moveTo(x, y)
        }
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, 50))
            .build()
        dispatchGesture(gesture, null, null)
    }

    private fun swipe(startX: Float, startY: Float, endX: Float, endY: Float) {
        val path = Path().apply {
            moveTo(startX, startY)
            lineTo(endX, endY)
        }
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, 200))
            .build()
        dispatchGesture(gesture, null, null)
    }
}
