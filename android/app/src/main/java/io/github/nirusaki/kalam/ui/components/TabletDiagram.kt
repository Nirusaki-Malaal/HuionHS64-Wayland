package io.github.nirusaki.kalam.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.unit.dp
import io.github.nirusaki.kalam.model.OrientationPreset
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

@Composable
fun TabletDiagram(
    activeButton: Int, // 0 = None, 1..4 = Pressed
    orientation: OrientationPreset = OrientationPreset.NORMAL,
    modifier: Modifier = Modifier
) {
    val bodyColor = MaterialTheme.colorScheme.surfaceContainerHighest
    val strokeColor = MaterialTheme.colorScheme.outline
    val activeKeyColor = MaterialTheme.colorScheme.primary
    val inactiveKeyColor = MaterialTheme.colorScheme.surfaceContainer

    val animatedRotation by animateFloatAsState(
        targetValue = orientation.degrees.toFloat(),
        animationSpec = tween(durationMillis = 350),
        label = "tabletRotation"
    )

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(220.dp)
    ) {
        Canvas(
            modifier = Modifier
                .fillMaxSize()
                .padding(8.dp)
        ) {
            val rad = Math.toRadians(animatedRotation.toDouble())
            val cosTheta = abs(cos(rad)).toFloat()
            val sinTheta = abs(sin(rad)).toFloat()
            val baseW = 560f
            val baseH = 350f

            // Bounding box dimensions for the rotated rectangle to guarantee zero cutoff
            val boundingW = baseW * cosTheta + baseH * sinTheta
            val boundingH = baseW * sinTheta + baseH * cosTheta

            val paddingPx = 16.dp.toPx()
            val availW = (size.width - paddingPx).coerceAtLeast(10f)
            val availH = (size.height - paddingPx).coerceAtLeast(10f)
            val scale = (availW / boundingW).coerceAtMost(availH / boundingH)

            val w = baseW * scale
            val h = baseH * scale
            val ox = -w / 2f
            val oy = -h / 2f

            rotate(degrees = animatedRotation, pivot = center) {
                translate(left = center.x, top = center.y) {
                    // Tablet chassis
                    drawRoundRect(
                        color = bodyColor,
                        topLeft = Offset(ox, oy),
                        size = Size(w, h),
                        cornerRadius = CornerRadius(16.dp.toPx() * (scale / 0.5f).coerceAtMost(1f))
                    )
                    drawRoundRect(
                        color = strokeColor,
                        topLeft = Offset(ox, oy),
                        size = Size(w, h),
                        cornerRadius = CornerRadius(16.dp.toPx() * (scale / 0.5f).coerceAtMost(1f)),
                        style = Stroke(width = 2.dp.toPx())
                    )

                    // Express Buttons: Button 1 to 4
                    val buttonCoords = listOf(
                        Offset(ox + 35f * scale, oy + 35f * scale),
                        Offset(ox + 35f * scale, oy + 85f * scale),
                        Offset(ox + (w - 75f * scale), oy + 35f * scale),
                        Offset(ox + (w - 75f * scale), oy + 85f * scale)
                    )

                    buttonCoords.forEachIndexed { idx, pos ->
                        val isPressed = (activeButton == idx + 1)
                        drawRoundRect(
                            color = if (isPressed) activeKeyColor else inactiveKeyColor,
                            topLeft = pos,
                            size = Size(40f * scale, 35f * scale),
                            cornerRadius = CornerRadius(4.dp.toPx())
                        )
                    }

                    // Active Area bounds
                    drawRoundRect(
                        color = strokeColor,
                        topLeft = Offset(ox + 90f * scale, oy + 120f * scale),
                        size = Size(w - 180f * scale, h - 145f * scale),
                        cornerRadius = CornerRadius(4.dp.toPx()),
                        style = Stroke(width = 1.dp.toPx())
                    )
                }
            }
        }
    }
}
