package io.github.nirusaki.kalam.data

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.*
import android.os.Build
import io.github.nirusaki.kalam.bridge.NativeBridge
import io.github.nirusaki.kalam.model.ExpressKeysReport
import io.github.nirusaki.kalam.model.StylusReport
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class UsbRepository(private val context: Context) {
    private val usbManager = context.getSystemService(Context.USB_SERVICE) as? UsbManager
    private val ACTION_USB_PERMISSION = "io.github.nirusaki.kalam.USB_PERMISSION"

    private val _isConnected = MutableStateFlow(false)
    val isConnected: StateFlow<Boolean> = _isConnected.asStateFlow()

    private val _stylusReport = MutableStateFlow<StylusReport?>(null)
    val stylusReport: StateFlow<StylusReport?> = _stylusReport.asStateFlow()

    private val _keyReport = MutableStateFlow(ExpressKeysReport(false, false, false, false))
    val keyReport: StateFlow<ExpressKeysReport> = _keyReport.asStateFlow()

    private var connection: UsbDeviceConnection? = null
    private var usbInterface: UsbInterface? = null
    private var inEndpoint: UsbEndpoint? = null
    private var pollJob: Job? = null
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(c: Context, intent: Intent) {
            try {
                when (intent.action) {
                    ACTION_USB_PERMISSION -> {
                        val device = getUsbDeviceFromIntent(intent)
                        val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                        if (granted && device != null) {
                            openDevice(device)
                        }
                    }
                    UsbManager.ACTION_USB_DEVICE_DETACHED -> {
                        val device = getUsbDeviceFromIntent(intent)
                        if (device?.vendorId == 0x256C) {
                            disconnectDevice()
                        }
                    }
                    UsbManager.ACTION_USB_DEVICE_ATTACHED -> {
                        probeDevice()
                    }
                }
            } catch (t: Throwable) {
                t.printStackTrace()
            }
        }
    }

    init {
        try {
            val filter = IntentFilter().apply {
                addAction(ACTION_USB_PERMISSION)
                addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                @Suppress("UnspecifiedRegisterReceiverFlag")
                context.registerReceiver(receiver, filter)
            }
        } catch (t: Throwable) {
            t.printStackTrace()
        }
        probeDevice()
    }

    private fun getUsbDeviceFromIntent(intent: Intent): UsbDevice? {
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(UsbManager.EXTRA_DEVICE)
            }
        } catch (_: Throwable) {
            null
        }
    }

    fun probeDevice() {
        val manager = usbManager ?: return
        try {
            val device = manager.deviceList.values.firstOrNull { it.vendorId == 0x256C }
            if (device != null) {
                if (manager.hasPermission(device)) {
                    openDevice(device)
                } else {
                    val intent = Intent(ACTION_USB_PERMISSION).apply {
                        setPackage(context.packageName)
                    }
                    val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        PendingIntent.FLAG_MUTABLE
                    } else {
                        0
                    }
                    val pi = PendingIntent.getBroadcast(context, 0, intent, flags)
                    manager.requestPermission(device, pi)
                }
            } else {
                disconnectDevice()
            }
        } catch (t: Throwable) {
            t.printStackTrace()
        }
    }

    private fun openDevice(device: UsbDevice) {
        val manager = usbManager ?: return
        try {
            val intf = (0 until device.interfaceCount)
                .map { device.getInterface(it) }
                .firstOrNull { it.interfaceClass == UsbConstants.USB_CLASS_HID }
                ?: device.getInterface(0)

            val ep = (0 until intf.endpointCount)
                .map { intf.getEndpoint(it) }
                .firstOrNull { it.direction == UsbConstants.USB_DIR_IN }
                ?: return

            val conn = manager.openDevice(device) ?: return
            if (!conn.claimInterface(intf, true)) {
                conn.close()
                return
            }

            connection = conn
            usbInterface = intf
            inEndpoint = ep
            _isConnected.value = true

            startPolling()
        } catch (t: Throwable) {
            t.printStackTrace()
        }
    }

    private fun startPolling() {
        pollJob?.cancel()
        pollJob = scope.launch {
            val buffer = ByteArray(64)
            val parsedFields = IntArray(7)

            while (isActive && _isConnected.value) {
                val conn = connection ?: break
                val ep = inEndpoint ?: break
                try {
                    val len = conn.bulkTransfer(ep, buffer, buffer.size, 100)
                    if (len > 0) {
                        val slice = buffer.copyOf(len)
                        val ok = NativeBridge.parseUsbReport(slice, parsedFields)
                        if (ok) {
                            when (parsedFields[0]) {
                                1 -> { // Stylus Report
                                    _stylusReport.value = StylusReport(
                                        x = parsedFields[1],
                                        y = parsedFields[2],
                                        pressure = parsedFields[3],
                                        touching = parsedFields[4] == 1,
                                        barrelSwitch = parsedFields[5] == 1,
                                        inRange = parsedFields[6] == 1
                                    )
                                }
                                2 -> { // Express Keys Report
                                    _keyReport.value = ExpressKeysReport(
                                        key1 = parsedFields[1] == 1,
                                        key2 = parsedFields[2] == 1,
                                        key3 = parsedFields[3] == 1,
                                        key4 = parsedFields[4] == 1
                                    )
                                }
                            }
                        }
                    }
                } catch (t: Throwable) {
                    if (t is CancellationException) throw t
                    t.printStackTrace()
                }
            }
        }
    }

    private fun disconnectDevice() {
        pollJob?.cancel()
        usbInterface?.let {
            try {
                connection?.releaseInterface(it)
            } catch (_: Throwable) {}
        }
        try {
            connection?.close()
        } catch (_: Throwable) {}
        connection = null
        usbInterface = null
        inEndpoint = null
        _isConnected.value = false
        _stylusReport.value = null
        _keyReport.value = ExpressKeysReport(false, false, false, false)
    }

    fun release() {
        disconnectDevice()
        scope.cancel()
        try {
            context.unregisterReceiver(receiver)
        } catch (_: Throwable) {}
    }
}
