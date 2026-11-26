package com.mouseshare.client

import java.io.InputStream
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.nio.ByteBuffer
import java.nio.ByteOrder

class NetworkClient(
    private val host: String,
    private val port: Int,
    private val screenWidth: Int,
    private val screenHeight: Int,
    private val listener: EventListener
) {

    interface EventListener {
        fun onConnected()
        fun onDisconnected(reason: String)
        fun onMouseMove(x: Int, y: Int)
        fun onMouseButton(button: Int, isDown: Boolean)
        fun onMouseWheel(delta: Int)
        fun onKey(vk: Int, isDown: Boolean)
        fun onClipboardText(text: String)
    }

    @Volatile
    var isConnected = false
        private set

    @Volatile
    private var isRunning = false
    private var socket: Socket? = null
    private var thread: Thread? = null

    fun start() {
        isRunning = true
        thread = Thread {
            runLoop()
        }.apply { start() }
    }

    fun stop() {
        isRunning = false
        try {
            socket?.close()
        } catch (_: Exception) {}
        thread?.interrupt()
        thread = null
        isConnected = false
    }

    private fun runLoop() {
        try {
            val sock = Socket()
            sock.tcpNoDelay = true
            sock.connect(InetSocketAddress(host, port), 5000)
            socket = sock

            val output = sock.getOutputStream()
            val input = sock.getInputStream()

            // Send Handshake: Packet ID 1, width: i32 (little endian), height: i32 (little endian)
            val handshakeBuf = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
            handshakeBuf.put(1.toByte())
            handshakeBuf.putInt(screenWidth)
            handshakeBuf.putInt(screenHeight)
            output.write(handshakeBuf.array())
            output.flush()

            // Read HandshakeAck (16 bytes)
            val ackBuf = ByteArray(16)
            readFully(input, ackBuf)
            if (ackBuf[0].toInt() != 2) {
                listener.onDisconnected("Handshake không hợp lệ")
                return
            }

            isConnected = true
            listener.onConnected()

            val packetBuf = ByteArray(16)

            while (isRunning) {
                val idByte = input.read()
                if (idByte == -1) break
                val pktId = idByte

                when (pktId) {
                    3 -> { // MouseMoveAbs
                        readFully(input, packetBuf, 0, 15)
                        val bb = ByteBuffer.wrap(packetBuf, 0, 8).order(ByteOrder.LITTLE_ENDIAN)
                        val x = bb.int
                        val y = bb.int
                        listener.onMouseMove(x, y)
                    }
                    4 -> { // MouseButton
                        readFully(input, packetBuf, 0, 15)
                        val button = packetBuf[0].toInt()
                        val isDown = packetBuf[1].toInt() != 0
                        listener.onMouseButton(button, isDown)
                    }
                    5 -> { // MouseWheel
                        readFully(input, packetBuf, 0, 15)
                        val bb = ByteBuffer.wrap(packetBuf, 0, 2).order(ByteOrder.LITTLE_ENDIAN)
                        val delta = bb.short.toInt()
                        listener.onMouseWheel(delta)
                    }
                    6 -> { // Key
                        readFully(input, packetBuf, 0, 15)
                        val bb = ByteBuffer.wrap(packetBuf, 0, 10).order(ByteOrder.LITTLE_ENDIAN)
                        val vk = bb.short.toInt() and 0xFFFF
                        val scan = bb.short.toInt() and 0xFFFF
                        val flags = bb.int
                        val isDown = packetBuf[8].toInt() != 0
                        listener.onKey(vk, isDown)
                    }
                    10 -> { // ClipboardText
                        val lenBuf = ByteArray(4)
                        readFully(input, lenBuf)
                        val len = ByteBuffer.wrap(lenBuf).order(ByteOrder.LITTLE_ENDIAN).int
                        if (len in 1..5_000_000) {
                            val textBuf = ByteArray(len)
                            readFully(input, textBuf)
                            val text = String(textBuf, Charsets.UTF_8)
                            listener.onClipboardText(text)
                        }
                    }
                    11 -> { // ClipboardFile (Skip data on Android mobile client to conserve memory)
                        val hdr = ByteArray(6)
                        readFully(input, hdr)
                        val bb = ByteBuffer.wrap(hdr).order(ByteOrder.LITTLE_ENDIAN)
                        val nameLen = bb.short.toInt() and 0xFFFF
                        val dataLen = bb.int
                        skipFully(input, nameLen.toLong())
                        skipFully(input, dataLen.toLong())
                    }
                    else -> {
                        // 1, 2, 7, 8 etc. read remaining 15 bytes
                        readFully(input, packetBuf, 0, 15)
                    }
                }
            }

        } catch (e: Exception) {
            if (isRunning) {
                listener.onDisconnected("Lỗi kết nối: ${e.message}")
            }
        } finally {
            isConnected = false
            try { socket?.close() } catch (_: Exception) {}
        }
    }

    private fun readFully(stream: InputStream, b: ByteArray, off: Int = 0, len: Int = b.size) {
        var n = 0
        while (n < len) {
            val count = stream.read(b, off + n, len - n)
            if (count < 0) throw java.io.EOFException()
            n += count
        }
    }

    private fun skipFully(stream: InputStream, n: Long) {
        var remaining = n
        while (remaining > 0) {
            val skipped = stream.skip(remaining)
            if (skipped <= 0) {
                if (stream.read() == -1) throw java.io.EOFException()
                remaining -= 1
            } else {
                remaining -= skipped
            }
        }
    }
}
