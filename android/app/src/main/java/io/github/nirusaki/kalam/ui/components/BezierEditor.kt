package io.github.nirusaki.kalam.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import io.github.nirusaki.kalam.bridge.NativeBridge

@Composable
fun BezierEditor(
    p1x: Float,
    p1y: Float,
    p2x: Float,
    p2y: Float,
    onCurveChange: (Float, Float, Float, Float) -> Unit,
    modifier: Modifier = Modifier
) {
    val lut = remember { FloatArray(256) }
    LaunchedEffect(p1x, p1y, p2x, p2y) {
        NativeBridge.sampleCurveLUT(p1x, p1y, p2x, p2y, lut)
    }

    val primaryColor = MaterialTheme.colorScheme.primary
    val gridColor = MaterialTheme.colorScheme.outlineVariant

    Column(modifier = modifier) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(180.dp)
                .background(
                    MaterialTheme.colorScheme.surfaceContainerHighest,
                    MaterialTheme.shapes.medium
                )
        ) {
            Canvas(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp)
            ) {
                val w = size.width
                val h = size.height

                // Draw linear baseline reference
                drawLine(
                    color = gridColor,
                    start = Offset(0f, h),
                    end = Offset(w, 0f),
                    strokeWidth = 2.dp.toPx(),
                    cap = StrokeCap.Round
                )

                // Plot Rust Bézier LUT
                val path = Path().apply {
                    moveTo(0f, h - (lut[0] * h))
                    for (i in 1 until 256) {
                        val x = (i / 255.0f) * w
                        val y = h - (lut[i] * h)
                        lineTo(x, y)
                    }
                }

                drawPath(
                    path = path,
                    color = primaryColor,
                    style = Stroke(width = 4.dp.toPx(), cap = StrokeCap.Round)
                )

                // Draw control point handles
                drawCircle(
                    color = primaryColor,
                    radius = 6.dp.toPx(),
                    center = Offset(p1x * w, h - (p1y * h))
                )
                drawCircle(
                    color = primaryColor,
                    radius = 6.dp.toPx(),
                    center = Offset(p2x * w, h - (p2y * h))
                )
            }
        }

        Spacer(modifier = Modifier.height(8.dp))
        Text(
            "Point 1: (${"%.2f".format(p1x)}, ${"%.2f".format(p1y)})",
            style = MaterialTheme.typography.labelMedium
        )
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Slider(
                value = p1x,
                onValueChange = { onCurveChange(it, p1y, p2x, p2y) },
                modifier = Modifier.weight(1f)
            )
            Slider(
                value = p1y,
                onValueChange = { onCurveChange(p1x, it, p2x, p2y) },
                modifier = Modifier.weight(1f)
            )
        }

        Text(
            "Point 2: (${"%.2f".format(p2x)}, ${"%.2f".format(p2y)})",
            style = MaterialTheme.typography.labelMedium
        )
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Slider(
                value = p2x,
                onValueChange = { onCurveChange(p1x, p1y, it, p2y) },
                modifier = Modifier.weight(1f)
            )
            Slider(
                value = p2y,
                onValueChange = { onCurveChange(p1x, p1y, p2x, it) },
                modifier = Modifier.weight(1f)
            )
        }
    }
}
