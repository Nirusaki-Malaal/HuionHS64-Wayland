package io.github.nirusaki.kalam.ui.screens

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import io.github.nirusaki.kalam.bridge.NativeBridge
import io.github.nirusaki.kalam.model.OrientationPreset
import io.github.nirusaki.kalam.model.StylusReport
import io.github.nirusaki.kalam.model.TabletSettings
import io.github.nirusaki.kalam.ui.components.PressureGauge

data class CanvasPoint(val offset: Offset, val pressure: Float)

@Composable
fun TestCanvasScreen(
    usbStylus: StylusReport?,
    settings: TabletSettings,
    modifier: Modifier = Modifier
) {
    val strokes = remember { mutableStateListOf<List<CanvasPoint>>() }
    var currentStroke by remember { mutableStateOf<MutableList<CanvasPoint>?>(null) }
    var liveNormalizedPressure by remember { mutableFloatStateOf(0f) }
    var rawLivePressure by remember { mutableIntStateOf(0) }

    // Map USB hardware pressure via JNI if connected
    LaunchedEffect(usbStylus) {
        usbStylus?.let { s ->
            if (s.inRange || s.touching) {
                rawLivePressure = s.pressure
                val shaped = NativeBridge.applyPressureCurve(
                    s.pressure,
                    settings.customP1x,
                    settings.customP1y,
                    settings.customP2x,
                    settings.customP2y
                )
                liveNormalizedPressure = shaped / 8191.0f
            } else {
                liveNormalizedPressure = 0f
                rawLivePressure = 0
            }
        }
    }

    val strokeColor = MaterialTheme.colorScheme.primary
    val gridColor = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(16.dp)
    ) {
        PressureGauge(
            normalizedPressure = liveNormalizedPressure,
            rawPressure = rawLivePressure
        )

        Spacer(modifier = Modifier.height(8.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "Draw with stylus or finger. Stroke width scales with pressure.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f)
            )
            FilledTonalButton(
                onClick = { strokes.clear() },
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)
            ) {
                Text("Clear")
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        // Drawing Area
        Box(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .background(
                    MaterialTheme.colorScheme.surfaceContainerLow,
                    MaterialTheme.shapes.large
                )
                .pointerInput(settings) {
                    awaitPointerEventScope {
                        while (true) {
                            val event = awaitPointerEvent()
                            val change = event.changes.firstOrNull() ?: continue
                            val pos = change.position

                            // Read pointer pressure if stylus-driven on Android screen
                            val pointerPressure = if (change.type == PointerType.Stylus && change.pressure > 0f) {
                                val raw8191 = (change.pressure * 8191f).toInt()
                                rawLivePressure = raw8191
                                val shaped = NativeBridge.applyPressureCurve(
                                    raw8191,
                                    settings.customP1x,
                                    settings.customP1y,
                                    settings.customP2x,
                                    settings.customP2y
                                )
                                liveNormalizedPressure = shaped / 8191f
                                liveNormalizedPressure
                            } else {
                                if (liveNormalizedPressure > 0.05f) liveNormalizedPressure else 0.4f
                            }

                            if (change.pressed) {
                                if (currentStroke == null) {
                                    val newStroke = mutableListOf(CanvasPoint(pos, pointerPressure))
                                    currentStroke = newStroke
                                    strokes.add(newStroke)
                                } else {
                                    currentStroke?.add(CanvasPoint(pos, pointerPressure))
                                }
                                change.consume()
                            } else {
                                currentStroke = null
                                liveNormalizedPressure = 0f
                                rawLivePressure = 0
                            }
                        }
                    }
                }
        ) {
            Canvas(modifier = Modifier.fillMaxSize().padding(8.dp)) {
                // Background subtle dot grid
                val dotSpacing = 24.dp.toPx()
                var x = dotSpacing
                while (x < size.width) {
                    var y = dotSpacing
                    while (y < size.height) {
                        drawCircle(color = gridColor, radius = 1.dp.toPx(), center = Offset(x, y))
                        y += dotSpacing
                    }
                    x += dotSpacing
                }

                // Render stroke paths with variable width
                for (stroke in strokes) {
                    if (stroke.size < 2) {
                        if (stroke.size == 1) {
                            val p = stroke[0]
                            drawCircle(
                                color = strokeColor,
                                radius = 2.dp.toPx() + 10.dp.toPx() * p.pressure,
                                center = p.offset
                            )
                        }
                        continue
                    }
                    for (i in 0 until stroke.size - 1) {
                        val p1 = stroke[i]
                        val p2 = stroke[i + 1]
                        val avgPressure = (p1.pressure + p2.pressure) / 2.0f
                        val width = 2.dp.toPx() + 18.dp.toPx() * avgPressure
                        drawLine(
                            color = strokeColor,
                            start = p1.offset,
                            end = p2.offset,
                            strokeWidth = width,
                            cap = StrokeCap.Round
                        )
                    }
                }
            }
        }
    }
}
