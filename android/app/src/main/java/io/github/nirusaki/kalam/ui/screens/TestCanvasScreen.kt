package io.github.nirusaki.kalam.ui.screens

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import io.github.nirusaki.kalam.bridge.NativeBridge
import io.github.nirusaki.kalam.model.ExpressKeysReport
import io.github.nirusaki.kalam.model.OrientationPreset
import io.github.nirusaki.kalam.model.StylusReport
import io.github.nirusaki.kalam.model.TabletSettings
import io.github.nirusaki.kalam.ui.components.PressureGauge
import kotlinx.coroutines.delay

data class CanvasPoint(val offset: Offset, val pressure: Float)

private enum class ButtonEdge { LEFT, TOP, RIGHT, BOTTOM }

@Composable
fun TestCanvasScreen(
    usbStylus: StylusReport?,
    keyReport: ExpressKeysReport? = null,
    settings: TabletSettings,
    modifier: Modifier = Modifier
) {
    val strokes = remember { mutableStateListOf<List<CanvasPoint>>() }
    val undoneStrokes = remember { mutableStateListOf<List<CanvasPoint>>() }
    var currentStroke by remember { mutableStateOf<MutableList<CanvasPoint>?>(null) }
    var liveNormalizedPressure by remember { mutableFloatStateOf(0f) }
    var rawLivePressure by remember { mutableIntStateOf(0) }
    var canvasSize by remember { mutableStateOf(IntSize.Zero) }
    var lastTriggeredAction by remember { mutableStateOf<String?>(null) }

    fun executeAction(buttonIndex: Int, action: String) {
        lastTriggeredAction = "[K$buttonIndex] $action"
        if (action.contains("Undo", ignoreCase = true)) {
            if (strokes.isNotEmpty()) {
                undoneStrokes.add(strokes.removeAt(strokes.lastIndex))
            }
        } else if (action.contains("Redo", ignoreCase = true)) {
            if (undoneStrokes.isNotEmpty()) {
                strokes.add(undoneStrokes.removeAt(undoneStrokes.lastIndex))
            }
        } else if (action.contains("Eraser", ignoreCase = true) || action.contains("Clear", ignoreCase = true)) {
            if (strokes.isNotEmpty()) {
                strokes.clear()
                undoneStrokes.clear()
            }
        }
    }

    // Express keys handling from hardware
    LaunchedEffect(keyReport) {
        val activeKey = keyReport?.activeKeyIndex() ?: 0
        if (activeKey > 0) {
            val action = when (activeKey) {
                1 -> settings.button1Action
                2 -> settings.button2Action
                3 -> settings.button3Action
                4 -> settings.button4Action
                else -> ""
            }
            executeAction(activeKey, action)
            delay(2000)
            if (lastTriggeredAction == "[K$activeKey] $action") {
                lastTriggeredAction = null
            }
        }
    }

    val isPortrait = settings.orientation == OrientationPreset.ROTATE_90 ||
            settings.orientation == OrientationPreset.ROTATE_270

    val buttonEdge = when (settings.orientation) {
        OrientationPreset.NORMAL -> ButtonEdge.LEFT
        OrientationPreset.ROTATE_90 -> ButtonEdge.TOP
        OrientationPreset.ROTATE_180 -> ButtonEdge.RIGHT
        OrientationPreset.ROTATE_270 -> ButtonEdge.BOTTOM
    }

    // Map USB hardware coordinates and pressure via JNI if directly connected via OTG
    LaunchedEffect(usbStylus, canvasSize, settings.orientation) {
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

                if (canvasSize.width > 0 && canvasSize.height > 0) {
                    val outCoords = IntArray(2)
                    NativeBridge.transformCoordinates(s.x, s.y, settings.orientation.degrees, outCoords)
                    val maxX = if (isPortrait) 25000f else 40000f
                    val maxY = if (isPortrait) 40000f else 25000f
                    val tx = (outCoords[0].toFloat() / maxX) * canvasSize.width
                    val ty = (outCoords[1].toFloat() / maxY) * canvasSize.height
                    val mappedOffset = Offset(
                        tx.coerceIn(0f, canvasSize.width.toFloat()),
                        ty.coerceIn(0f, canvasSize.height.toFloat())
                    )

                    if (s.touching) {
                        if (currentStroke == null) {
                            val newStroke = mutableListOf(CanvasPoint(mappedOffset, liveNormalizedPressure))
                            currentStroke = newStroke
                            strokes.add(newStroke)
                            undoneStrokes.clear()
                        } else {
                            currentStroke?.add(CanvasPoint(mappedOffset, liveNormalizedPressure))
                        }
                    } else {
                        currentStroke = null
                    }
                }
            } else {
                liveNormalizedPressure = 0f
                rawLivePressure = 0
                currentStroke = null
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
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Draw with stylus or finger. Stroke width scales with pressure.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = "Canvas active: ${settings.orientation.label} · Buttons on ${buttonEdge.name}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilledTonalIconButton(
                    onClick = {
                        if (strokes.isNotEmpty()) {
                            undoneStrokes.add(strokes.removeAt(strokes.lastIndex))
                        }
                    },
                    enabled = strokes.isNotEmpty()
                ) {
                    Icon(Icons.Default.Refresh, contentDescription = "Undo")
                }
                FilledTonalButton(
                    onClick = {
                        strokes.clear()
                        undoneStrokes.clear()
                    },
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)
                ) {
                    Icon(Icons.Default.Delete, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Clear")
                }
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        // Tablet Frame with rotated canvas and hardware button positions
        BoxWithConstraints(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            contentAlignment = Alignment.Center
        ) {
            val canvasAspectRatio = if (isPortrait) 10f / 16f else 16f / 10f
            val isHorizontal = buttonEdge == ButtonEdge.LEFT || buttonEdge == ButtonEdge.RIGHT

            // Allocate space for express buttons strip
            val availW = if (isHorizontal) (maxWidth - 120.dp).coerceAtLeast(120.dp) else maxWidth
            val availH = if (!isHorizontal) (maxHeight - 64.dp).coerceAtLeast(120.dp) else maxHeight

            val targetW: androidx.compose.ui.unit.Dp
            val targetH: androidx.compose.ui.unit.Dp
            if (availW / availH > canvasAspectRatio) {
                targetH = availH
                targetW = availH * canvasAspectRatio
            } else {
                targetW = availW
                targetH = availW / canvasAspectRatio
            }

            when (buttonEdge) {
                ButtonEdge.LEFT -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.Center
                    ) {
                        ExpressButtonsStrip(
                            isVertical = true,
                            activeButton = keyReport?.activeKeyIndex() ?: 0,
                            settings = settings,
                            onButtonClicked = { idx, act -> executeAction(idx, act) }
                        )
                        Spacer(modifier = Modifier.width(12.dp))
                        CanvasContainer(
                            modifier = Modifier.size(targetW, targetH),
                            settings = settings,
                            gridColor = gridColor,
                            strokeColor = strokeColor,
                            strokes = strokes,
                            undoneStrokes = undoneStrokes,
                            currentStroke = currentStroke,
                            onCurrentStrokeChange = { currentStroke = it },
                            onSizeChanged = { canvasSize = it },
                            onPointerPressureChange = { raw, norm ->
                                rawLivePressure = raw
                                liveNormalizedPressure = norm
                            },
                            liveNormalizedPressure = liveNormalizedPressure,
                            lastTriggeredAction = lastTriggeredAction
                        )
                    }
                }
                ButtonEdge.RIGHT -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.Center
                    ) {
                        CanvasContainer(
                            modifier = Modifier.size(targetW, targetH),
                            settings = settings,
                            gridColor = gridColor,
                            strokeColor = strokeColor,
                            strokes = strokes,
                            undoneStrokes = undoneStrokes,
                            currentStroke = currentStroke,
                            onCurrentStrokeChange = { currentStroke = it },
                            onSizeChanged = { canvasSize = it },
                            onPointerPressureChange = { raw, norm ->
                                rawLivePressure = raw
                                liveNormalizedPressure = norm
                            },
                            liveNormalizedPressure = liveNormalizedPressure,
                            lastTriggeredAction = lastTriggeredAction
                        )
                        Spacer(modifier = Modifier.width(12.dp))
                        ExpressButtonsStrip(
                            isVertical = true,
                            activeButton = keyReport?.activeKeyIndex() ?: 0,
                            settings = settings,
                            onButtonClicked = { idx, act -> executeAction(idx, act) }
                        )
                    }
                }
                ButtonEdge.TOP -> {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center
                    ) {
                        ExpressButtonsStrip(
                            isVertical = false,
                            activeButton = keyReport?.activeKeyIndex() ?: 0,
                            settings = settings,
                            onButtonClicked = { idx, act -> executeAction(idx, act) }
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        CanvasContainer(
                            modifier = Modifier.size(targetW, targetH),
                            settings = settings,
                            gridColor = gridColor,
                            strokeColor = strokeColor,
                            strokes = strokes,
                            undoneStrokes = undoneStrokes,
                            currentStroke = currentStroke,
                            onCurrentStrokeChange = { currentStroke = it },
                            onSizeChanged = { canvasSize = it },
                            onPointerPressureChange = { raw, norm ->
                                rawLivePressure = raw
                                liveNormalizedPressure = norm
                            },
                            liveNormalizedPressure = liveNormalizedPressure,
                            lastTriggeredAction = lastTriggeredAction
                        )
                    }
                }
                ButtonEdge.BOTTOM -> {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center
                    ) {
                        CanvasContainer(
                            modifier = Modifier.size(targetW, targetH),
                            settings = settings,
                            gridColor = gridColor,
                            strokeColor = strokeColor,
                            strokes = strokes,
                            undoneStrokes = undoneStrokes,
                            currentStroke = currentStroke,
                            onCurrentStrokeChange = { currentStroke = it },
                            onSizeChanged = { canvasSize = it },
                            onPointerPressureChange = { raw, norm ->
                                rawLivePressure = raw
                                liveNormalizedPressure = norm
                            },
                            liveNormalizedPressure = liveNormalizedPressure,
                            lastTriggeredAction = lastTriggeredAction
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                        ExpressButtonsStrip(
                            isVertical = false,
                            activeButton = keyReport?.activeKeyIndex() ?: 0,
                            settings = settings,
                            onButtonClicked = { idx, act -> executeAction(idx, act) }
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ExpressButtonsStrip(
    isVertical: Boolean,
    activeButton: Int,
    settings: TabletSettings,
    onButtonClicked: (Int, String) -> Unit,
    modifier: Modifier = Modifier
) {
    val buttons = listOf(
        Triple(1, "K1", settings.button1Action),
        Triple(2, "K2", settings.button2Action),
        Triple(3, "K3", settings.button3Action),
        Triple(4, "K4", settings.button4Action)
    )

    if (isVertical) {
        Column(
            modifier = modifier
                .background(
                    MaterialTheme.colorScheme.surfaceContainerHighest,
                    MaterialTheme.shapes.medium
                )
                .padding(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text("HS64", style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
            buttons.forEach { (idx, label, action) ->
                val isPressed = activeButton == idx
                FilledTonalButton(
                    onClick = { onButtonClicked(idx, action) },
                    colors = ButtonDefaults.filledTonalButtonColors(
                        containerColor = if (isPressed) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainer,
                        contentColor = if (isPressed) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface
                    ),
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                    modifier = Modifier.width(88.dp)
                ) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(label, fontWeight = FontWeight.Bold, style = MaterialTheme.typography.labelMedium)
                        Text(
                            action.substringBefore("·").trim(),
                            style = MaterialTheme.typography.labelSmall,
                            maxLines = 1
                        )
                    }
                }
            }
        }
    } else {
        Row(
            modifier = modifier
                .background(
                    MaterialTheme.colorScheme.surfaceContainerHighest,
                    MaterialTheme.shapes.medium
                )
                .padding(8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("HS64", style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold, modifier = Modifier.padding(end = 4.dp))
            buttons.forEach { (idx, label, action) ->
                val isPressed = activeButton == idx
                FilledTonalButton(
                    onClick = { onButtonClicked(idx, action) },
                    colors = ButtonDefaults.filledTonalButtonColors(
                        containerColor = if (isPressed) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainer,
                        contentColor = if (isPressed) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface
                    ),
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(label, fontWeight = FontWeight.Bold, style = MaterialTheme.typography.labelMedium)
                        Spacer(modifier = Modifier.width(4.dp))
                        Text(
                            action.substringBefore("·").trim(),
                            style = MaterialTheme.typography.labelSmall
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun CanvasContainer(
    settings: TabletSettings,
    gridColor: androidx.compose.ui.graphics.Color,
    strokeColor: androidx.compose.ui.graphics.Color,
    strokes: MutableList<List<CanvasPoint>>,
    undoneStrokes: MutableList<List<CanvasPoint>>,
    currentStroke: MutableList<CanvasPoint>?,
    onCurrentStrokeChange: (MutableList<CanvasPoint>?) -> Unit,
    onSizeChanged: (IntSize) -> Unit,
    onPointerPressureChange: (Int, Float) -> Unit,
    liveNormalizedPressure: Float,
    lastTriggeredAction: String?,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .background(
                MaterialTheme.colorScheme.surfaceContainerLow,
                MaterialTheme.shapes.large
            )
            .border(
                width = 2.dp,
                color = MaterialTheme.colorScheme.outlineVariant,
                shape = MaterialTheme.shapes.large
            )
            .onSizeChanged { onSizeChanged(it) }
            .pointerInput(settings) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull() ?: continue
                        val rawPos = change.position

                        // Transform pointer coordinate based on selected tablet rotation
                        val pos = transformPointerOffset(
                            rawPos,
                            size = Size(size.width.toFloat(), size.height.toFloat()),
                            orientation = settings.orientation
                        )

                        // Read pointer pressure if stylus-driven on Android screen
                        val pointerPressure = if (change.type == PointerType.Stylus && change.pressure > 0f) {
                            val raw8191 = (change.pressure * 8191f).toInt()
                            val shaped = NativeBridge.applyPressureCurve(
                                raw8191,
                                settings.customP1x,
                                settings.customP1y,
                                settings.customP2x,
                                settings.customP2y
                            )
                            val norm = shaped / 8191f
                            onPointerPressureChange(raw8191, norm)
                            norm
                        } else {
                            if (liveNormalizedPressure > 0.05f) liveNormalizedPressure else 0.4f
                        }

                        if (change.pressed) {
                            if (currentStroke == null) {
                                val newStroke = mutableListOf(CanvasPoint(pos, pointerPressure))
                                onCurrentStrokeChange(newStroke)
                                strokes.add(newStroke)
                                undoneStrokes.clear()
                            } else {
                                currentStroke.add(CanvasPoint(pos, pointerPressure))
                            }
                            change.consume()
                        } else {
                            onCurrentStrokeChange(null)
                            onPointerPressureChange(0, 0f)
                        }
                    }
                }
            }
    ) {
        Canvas(modifier = Modifier.fillMaxSize().padding(12.dp)) {
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

        // HUD Pill showing triggered express key action
        lastTriggeredAction?.let { actionText ->
            Surface(
                shape = MaterialTheme.shapes.extraLarge,
                color = MaterialTheme.colorScheme.primaryContainer,
                tonalElevation = 6.dp,
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .padding(top = 16.dp)
            ) {
                Text(
                    text = actionText,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)
                )
            }
        }
    }
}

private fun transformPointerOffset(
    pos: Offset,
    size: Size,
    orientation: OrientationPreset
): Offset {
    if (size.width <= 0f || size.height <= 0f) return pos
    return when (orientation) {
        OrientationPreset.NORMAL -> pos
        OrientationPreset.ROTATE_90 -> Offset(
            x = size.width * (1f - (pos.y / size.height).coerceIn(0f, 1f)),
            y = size.height * ((pos.x / size.width).coerceIn(0f, 1f))
        )
        OrientationPreset.ROTATE_180 -> Offset(
            x = size.width - pos.x,
            y = size.height - pos.y
        )
        OrientationPreset.ROTATE_270 -> Offset(
            x = size.width * ((pos.y / size.height).coerceIn(0f, 1f)),
            y = size.height * (1f - (pos.x / size.width).coerceIn(0f, 1f))
        )
    }
}
