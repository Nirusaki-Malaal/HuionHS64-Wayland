package io.github.nirusaki.kalam.bridge

object NativeBridge {
    init {
        try {
            System.loadLibrary("huion_jni")
        } catch (e: UnsatisfiedLinkError) {
            System.err.println("Failed to load libhuion_jni.so: ${e.message}")
        }
    }

    external fun applyPressureCurve(
        rawPressure: Int,
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float
    ): Int

    external fun transformCoordinates(
        x: Int,
        y: Int,
        orientationDeg: Int,
        outCoords: IntArray
    )

    external fun parseUsbReport(
        rawBuffer: ByteArray,
        outPayload: IntArray
    ): Boolean

    external fun sampleCurveLUT(
        p1x: Float,
        p1y: Float,
        p2x: Float,
        p2y: Float,
        outSamples: FloatArray
    )
}
