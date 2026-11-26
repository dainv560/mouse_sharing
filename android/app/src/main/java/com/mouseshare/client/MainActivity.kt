package com.mouseshare.client

import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {

    private lateinit var editServerIp: EditText
    private lateinit var editPort: EditText
    private lateinit var btnConnect: Button
    private lateinit var btnAccessibility: Button
    private lateinit var txtStatus: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        editServerIp = findViewById(R.id.editServerIp)
        editPort = findViewById(R.id.editPort)
        btnConnect = findViewById(R.id.btnConnect)
        btnAccessibility = findViewById(R.id.btnAccessibility)
        txtStatus = findViewById(R.id.txtStatus)

        btnAccessibility.setOnClickListener {
            val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
            startActivity(intent)
            Toast.makeText(this, "Hãy tìm và bật 'Mouse Share Service'", Toast.LENGTH_LONG).show()
        }

        btnConnect.setOnClickListener {
            val ip = editServerIp.text.toString().trim()
            val port = editPort.text.toString().toIntOrNull() ?: 8421

            if (ip.isEmpty()) {
                Toast.makeText(this, "Vui lòng nhập địa chỉ IP của Máy Chủ PC", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }

            val service = MouseShareService.instance
            if (service == null) {
                Toast.makeText(this, "Chưa bật Dịch vụ Trợ năng! Bấm nút bên dưới để cấp quyền.", Toast.LENGTH_LONG).show()
                return@setOnClickListener
            }

            if (service.isConnected()) {
                service.disconnect()
                btnConnect.text = "Kết Nối Tới PC"
                txtStatus.text = "Trạng thái: Đã ngắt kết nối"
            } else {
                txtStatus.text = "Đang kết nối tới $ip:$port..."
                service.connect(ip, port) { connected, msg ->
                    runOnUiThread {
                        if (connected) {
                            btnConnect.text = "Ngắt Kết Nối"
                            txtStatus.text = "Trạng thái: Đã kết nối! Chuột & phím từ PC đã sẵn sàng."
                        } else {
                            btnConnect.text = "Kết Nối Tới PC"
                            txtStatus.text = "Trạng thái: $msg"
                        }
                    }
                }
            }
        }
    }
}
