package io.github.nirusaki.kalam.bridge

import kotlin.math.roundToInt

object NativeBridge {
    private var isNativeLoaded: Boolean = false

    init {
        try {
            System.loadLibrary("huion_jni")
            isNativeLoaded = true
        } catch (_: Throwable) {
            isNativeLoaded = false
        }
    }

    private external fun nativeApplyPressureCurve(
        rawPressure: Int,
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float
    ): Int

    private external fun nativeTransformCoordinates(
        x: Int,
        y: Int,
        orientationDeg: Int,
        outCoords: IntArray
    )

    private external fun nativeParseUsbReport(
        rawBuffer: ByteArray,
        outPayload: IntArray
    ): Boolean

    private external fun nativeSampleCurveLUT(
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float,
        outSamples: FloatArray
    )

    fun applyPressureCurve(
        rawPressure: Int,
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float
    ): Int {
        if (isNativeLoaded) {
            try {
                return nativeApplyPressureCurve(rawPressure, p1x, p1y, p2x, p2y)
            } catch (_: UnsatisfiedLinkError) {
                isNativeLoaded = false
            }
        }
        val clamped = (rawPressure.coerceIn(0, 8191)).toDouble() / 8191.0
        val evaluated = evaluateBezier(clamped, p1x.toDouble(), p1y.toDouble(), p2x.toDouble(), p2y.toDouble())
        return (evaluated * 8191.0).roundToInt().coerceIn(0, 8191)
    }

    fun transformCoordinates(
        x: Int,
        y: Int,
        orientationDeg: Int,
        outCoords: IntArray
    ) {
        if (isNativeLoaded) {
            try {
                nativeTransformCoordinates(x, y, orientationDeg, outCoords)
                return
            } catch (_: UnsatisfiedLinkError) {
                isNativeLoaded = false
            }
        }
        val normX = x.coerceIn(0, 40000)
        val normY = y.coerceIn(0, 25000)
        val (tx, ty) = when (orientationDeg) {
            90 -> Pair(
                40000 - ((normY.toLong() * 40000) / 25000).toInt(),
                ((normX.toLong() * 25000) / 40000).toInt()
            )
            180 -> Pair(40000 - normX, 25000 - normY)
            270 -> Pair(
                ((normY.toLong() * 40000) / 25000).toInt(),
                25000 - ((normX.toLong() * 25000) / 40000).toInt()
            )
            else -> Pair(normX, normY)
        }
        outCoords[0] = tx.coerceIn(0, 40000)
        outCoords[1] = ty.coerceIn(0, 25000)
    }

    fun parseUsbReport(
        rawBuffer: ByteArray,
        outPayload: IntArray
    ): Boolean {
        if (isNativeLoaded) {
            try {
                return nativeParseUsbReport(rawBuffer, outPayload)
            } catch (_: UnsatisfiedLinkError) {
                isNativeLoaded = false
            }
        }
        if (rawBuffer.isEmpty() || outPayload.size < 7) return false
        val reportId = rawBuffer[0].toInt() and 0xFF
        return when (reportId) {
            0x07 -> { // Stylus hover
                if (rawBuffer.size < 8) return false
                val status = rawBuffer[1].toInt() and 0xFF
                val inRange = (status and 0x10) != 0
                val x = (rawBuffer[2].toInt() and 0xFF) or ((rawBuffer[3].toInt() and 0xFF) shl 8)
                val y = (rawBuffer[4].toInt() and 0xFF) or ((rawBuffer[5].toInt() and 0xFF) shl 8)
                val p = (rawBuffer[6].toInt() and 0xFF) or ((rawBuffer[7].toInt() and 0xFF) shl 8)
                outPayload[0] = 1
                outPayload[1] = x
                outPayload[2] = y
                outPayload[3] = p
                outPayload[4] = 0
                outPayload[5] = 0
                outPayload[6] = if (inRange) 1 else 0
                true
            }
            0x08 -> {
                if (rawBuffer.size < 8) return false
                val b1 = rawBuffer[1].toInt() and 0xFF
                if ((b1 and 0xF0) == 0xE0) { // Express keys
                    val mask = rawBuffer[4].toInt() and 0xFF
                    outPayload[0] = 2
                    outPayload[1] = if ((mask and 0x01) != 0) 1 else 0
                    outPayload[2] = if ((mask and 0x02) != 0) 1 else 0
                    outPayload[3] = if ((mask and 0x04) != 0) 1 else 0
                    outPayload[4] = if ((mask and 0x08) != 0) 1 else 0
                    outPayload[5] = 0
                    outPayload[6] = 0
                    true
                } else false
            }
            0x09 -> { // Stylus touch/barrel
                if (rawBuffer.size < 8) return false
                val status = rawBuffer[1].toInt() and 0xFF
                val touch = (status and 0x01) != 0
                val barrel = (status and 0x02) != 0
                val inRange = (status and 0x10) != 0 || touch
                val x = (rawBuffer[2].toInt() and 0xFF) or ((rawBuffer[3].toInt() and 0xFF) shl 8)
                val y = (rawBuffer[4].toInt() and 0xFF) or ((rawBuffer[5].toInt() and 0xFF) shl 8)
                val p = (rawBuffer[6].toInt() and 0xFF) or ((rawBuffer[7].toInt() and 0xFF) shl 8)
                outPayload[0] = 1
                outPayload[1] = x
                outPayload[2] = y
                outPayload[3] = p
                outPayload[4] = if (touch) 1 else 0
                outPayload[5] = if (barrel) 1 else 0
                outPayload[6] = if (inRange) 1 else 0
                true
            }
            0x0a -> { // Standard report
                if (rawBuffer.size < 10) return false
                val status = rawBuffer[1].toInt() and 0xFF
                val inRange = (status and 0x40) != 0
                val touch = (status and 0x01) != 0
                val barrel = (status and 0x02) != 0
                val rawX = (rawBuffer[2].toInt() and 0xFF) or ((rawBuffer[3].toInt() and 0xFF) shl 8)
                val rawY = (rawBuffer[4].toInt() and 0xFF) or ((rawBuffer[5].toInt() and 0xFF) shl 8)
                val p = (rawBuffer[6].toInt() and 0xFF) or ((rawBuffer[7].toInt() and 0xFF) shl 8)
                val x = ((rawX.toLong() * 40000) / 32000).toInt().coerceAtMost(40000)
                val y = ((rawY.toLong() * 25000) / 20400).toInt().coerceAtMost(25000)
                outPayload[0] = 1
                outPayload[1] = x
                outPayload[2] = y
                outPayload[3] = p
                outPayload[4] = if (touch) 1 else 0
                outPayload[5] = if (barrel) 1 else 0
                outPayload[6] = if (inRange) 1 else 0
                true
            }
            0x0e -> { // Express keys
                if (rawBuffer.size < 5) return false
                val mask = rawBuffer[4].toInt() and 0xFF
                outPayload[0] = 2
                outPayload[1] = if ((mask and 0x01) != 0) 1 else 0
                outPayload[2] = if ((mask and 0x02) != 0) 1 else 0
                outPayload[3] = if ((mask and 0x04) != 0) 1 else 0
                outPayload[4] = if ((mask and 0x08) != 0) 1 else 0
                outPayload[5] = 0
                outPayload[6] = 0
                true
            }
            else -> false
        }
    }

    fun sampleCurveLUT(
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float,
        outSamples: FloatArray
    ) {
        if (isNativeLoaded) {
            try {
                nativeSampleCurveLUT(p1x, p1y, p2x, p2y, outSamples)
                return
            } catch (_: UnsatisfiedLinkError) {
                isNativeLoaded = false
            }
        }
        for (i in 0 until 256.coerceAtMost(outSamples.size)) {
            val inputRatio = i.toDouble() / 255.0
            outSamples[i] = evaluateBezier(
                inputRatio,
                p1x.toDouble(),
                p1y.toDouble(),
                p2x.toDouble(),
                p2y.toDouble()
            ).toFloat()
        }
    }

    private fun bezierX(t: Double, p1x: Double, p2x: Double): Double {
        val u = 1.0 - t
        return 3.0 * u * u * t * p1x + 3.0 * u * t * t * p2x + t * t * t
    }

    private fun bezierY(t: Double, p1y: Double, p2y: Double): Double {
        val u = 1.0 - t
        return 3.0 * u * u * t * p1y + 3.0 * u * t * t * p2y + t * t * t
    }

    private fun evaluateBezier(input: Double, p1x: Double, p1y: Double, p2x: Double, p2y: Double): Double {
        val x = input.coerceIn(0.0, 1.0)
        if (x <= 0.0) return 0.0
        if (x >= 1.0) return 1.0

        var low = 0.0
        var high = 1.0
        var t = x

        for (i in 0 until 16) {
            val currentX = bezierX(t, p1x, p2x)
            if (kotlin.math.abs(currentX - x) < 1e-4) break
            if (currentX < x) {
                low = t
            } else {
                high = t
            }
            t = (low + high) / 2.0
        }
        return bezierY(t, p1y, p2y).coerceIn(0.0, 1.0)
    }
}
