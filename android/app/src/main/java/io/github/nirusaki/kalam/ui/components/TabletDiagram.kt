package io.github.nirusaki.kalam.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

@Composable
fun TabletDiagram(
    activeButton: Int, // 0 = None, 1..4 = Pressed
    modifier: Modifier = Modifier
) {
    val bodyColor = MaterialTheme.colorScheme.surfaceContainerHighest
    val strokeColor = MaterialTheme.colorScheme.outline
    val activeKeyColor = MaterialTheme.colorScheme.primary
    val inactiveKeyColor = MaterialTheme.colorScheme.surfaceContainer

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(160.dp)
    ) {
        Canvas(
            modifier = Modifier
                .fillMaxSize()
                .padding(12.dp)
        ) {
            val scale = (size.width / 560f).coerceAtMost(size.height / 350f)
            val w = 560f * scale
            val h = 350f * scale
            val ox = (size.width - w) / 2f
            val oy = (size.height - h) / 2f

            // Tablet chassis
            drawRoundRect(
                color = bodyColor,
                topLeft = Offset(ox, oy),
                size = Size(w, h),
                cornerRadius = CornerRadius(16.dp.toPx())
            )
            drawRoundRect(
                color = strokeColor,
                topLeft = Offset(ox, oy),
                size = Size(w, h),
                cornerRadius = CornerRadius(16.dp.toPx()),
                style = Stroke(width = 2.dp.toPx())
            )

            // Express Buttons: Button 1 to 4
            val buttonCoords = listOf(
                Offset(ox + 40f * scale, oy + 40f * scale),
                Offset(ox + 40f * scale, oy + 90f * scale),
                Offset(ox + (w - 80f * scale), oy + 40f * scale),
                Offset(ox + (w - 80f * scale), oy + 90f * scale)
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
                topLeft = Offset(ox + 100f * scale, oy + 130f * scale),
                size = Size(w - 200f * scale, h - 150f * scale),
                cornerRadius = CornerRadius(4.dp.toPx()),
                style = Stroke(width = 1.dp.toPx())
            )
        }
    }
}
