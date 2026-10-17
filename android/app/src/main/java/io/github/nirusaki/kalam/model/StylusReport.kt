package io.github.nirusaki.kalam.model

data class StylusReport(
    val x: Int,
    val y: Int,
    val pressure: Int,
    val touching: Boolean,
    val barrelSwitch: Boolean,
    val inRange: Boolean
)

data class ExpressKeysReport(
    val key1: Boolean,
    val key2: Boolean,
    val key3: Boolean,
    val key4: Boolean
) {
    fun activeKeyIndex(): Int {
        return when {
            key1 -> 1
            key2 -> 2
            key3 -> 3
            key4 -> 4
            else -> 0
        }
    }
}

enum class CurvePreset(
    val label: String,
    val p1x: Float,
    val p1y: Float,
    val p2x: Float,
    val p2y: Float
) {
    LINEAR("Linear", 0.25f, 0.25f, 0.75f, 0.75f),
    SOFT("Soft", 0.20f, 0.50f, 0.50f, 0.90f),
    FIRM("Firm", 0.50f, 0.10f, 0.80f, 0.50f),
    CUSTOM("Custom", 0.30f, 0.30f, 0.70f, 0.70f)
}

enum class OrientationPreset(val degrees: Int, val label: String) {
    NORMAL(0, "0° · Landscape"),
    ROTATE_90(90, "90° · Portrait"),
    ROTATE_180(180, "180° · Inverted"),
    ROTATE_270(270, "270° · Inverted Portrait")
}

data class TabletSettings(
    val preset: CurvePreset = CurvePreset.LINEAR,
    val customP1x: Float = 0.25f,
    val customP1y: Float = 0.25f,
    val customP2x: Float = 0.75f,
    val customP2y: Float = 0.75f,
    val orientation: OrientationPreset = OrientationPreset.NORMAL,
    val button1Action: String = "Undo · Ctrl+Z",
    val button2Action: String = "Redo · Ctrl+Shift+Z",
    val button3Action: String = "Brush · B",
    val button4Action: String = "Eraser · E"
)
