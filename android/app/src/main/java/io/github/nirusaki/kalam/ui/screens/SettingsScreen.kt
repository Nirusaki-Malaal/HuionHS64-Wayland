package io.github.nirusaki.kalam.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.github.nirusaki.kalam.model.CurvePreset
import io.github.nirusaki.kalam.model.OrientationPreset
import io.github.nirusaki.kalam.model.TabletSettings
import io.github.nirusaki.kalam.ui.components.BezierEditor
import io.github.nirusaki.kalam.ui.components.TabletDiagram

private val PRESET_SHORTCUTS = listOf(
    "Undo · Ctrl+Z",
    "Redo · Ctrl+Shift+Z",
    "Brush · B",
    "Eraser · E",
    "Hand / Pan · Space",
    "Zoom In · Ctrl++",
    "Zoom Out · Ctrl+-",
    "Eyedropper · I",
    "Save · Ctrl+S",
    "Next Tool · Tab"
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    isConnected: Boolean,
    connectionStatus: String,
    activeButton: Int,
    settings: TabletSettings,
    onSettingsChange: (TabletSettings) -> Unit,
    onRefreshUsb: () -> Unit,
    modifier: Modifier = Modifier
) {
    val scrollState = rememberScrollState()
    var editingButtonIndex by remember { mutableStateOf<Int?>(null) }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(scrollState)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        // Status Card
        ElevatedCard(modifier = Modifier.fillMaxWidth()) {
            Row(
                modifier = Modifier.padding(16.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Huion HS64", style = MaterialTheme.typography.titleMedium)
                    Text(
                        connectionStatus,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isConnected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error
                    )
                }
                FilledTonalButton(onClick = onRefreshUsb) {
                    Text("Detect")
                }
            }
        }

        // Hardware layout with live express-key feedback and visual orientation
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Tablet Hardware Reference", style = MaterialTheme.typography.titleSmall)
                    AssistChip(
                        onClick = {},
                        label = { Text(settings.orientation.label) }
                    )
                }
                Text(
                    "Hardware buttons 1-2 on left, 3-4 on right. Rotates live with selected orientation.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(12.dp))
                TabletDiagram(
                    activeButton = activeButton,
                    orientation = settings.orientation
                )
            }
        }

        // Pressure Curve Card
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text("Pressure Sensitivity", style = MaterialTheme.typography.titleSmall)
                Spacer(modifier = Modifier.height(8.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    CurvePreset.values().forEach { preset ->
                        FilterChip(
                            selected = settings.preset == preset,
                            onClick = {
                                onSettingsChange(
                                    settings.copy(
                                        preset = preset,
                                        customP1x = preset.p1x,
                                        customP1y = preset.p1y,
                                        customP2x = preset.p2x,
                                        customP2y = preset.p2y
                                    )
                                )
                            },
                            label = { Text(preset.label) }
                        )
                    }
                }

                Spacer(modifier = Modifier.height(12.dp))
                BezierEditor(
                    p1x = settings.customP1x,
                    p1y = settings.customP1y,
                    p2x = settings.customP2x,
                    p2y = settings.customP2y,
                    onCurveChange = { p1x, p1y, p2x, p2y ->
                        onSettingsChange(
                            settings.copy(
                                preset = CurvePreset.CUSTOM,
                                customP1x = p1x,
                                customP1y = p1y,
                                customP2x = p2x,
                                customP2y = p2y
                            )
                        )
                    }
                )
            }
        }

        // Orientation Card
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text("Tablet Rotation", style = MaterialTheme.typography.titleSmall)
                Text(
                    "Coordinates and chassis automatically map to selected angle.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(8.dp))
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OrientationPreset.values().forEach { orient ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            RadioButton(
                                selected = settings.orientation == orient,
                                onClick = { onSettingsChange(settings.copy(orientation = orient)) }
                            )
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(orient.label, style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                }
            }
        }

        // Express Buttons Mapping Card (Fully Interactive)
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Configured Express Keys", style = MaterialTheme.typography.titleSmall)
                    Text("Tap to edit", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
                }

                val buttons = listOf(
                    Triple(1, "Button 1 (Upper Left)", settings.button1Action),
                    Triple(2, "Button 2 (Lower Left)", settings.button2Action),
                    Triple(3, "Button 3 (Upper Right)", settings.button3Action),
                    Triple(4, "Button 4 (Lower Right)", settings.button4Action)
                )

                buttons.forEach { (index, title, currentAction) ->
                    val isPressed = (activeButton == index)
                    Surface(
                        onClick = { editingButtonIndex = index },
                        shape = MaterialTheme.shapes.medium,
                        color = if (isPressed) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerLow,
                        tonalElevation = if (isPressed) 4.dp else 1.dp,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Badge(
                                    containerColor = if (isPressed) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.secondaryContainer,
                                    contentColor = if (isPressed) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSecondaryContainer
                                ) {
                                    Text("K$index", modifier = Modifier.padding(4.dp))
                                }
                                Spacer(modifier = Modifier.width(12.dp))
                                Column {
                                    Text(title, style = MaterialTheme.typography.bodyMedium)
                                    Text(
                                        currentAction,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
                            }
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                if (isPressed) {
                                    Text(
                                        "PRESSED",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.primary,
                                        modifier = Modifier.padding(end = 8.dp)
                                    )
                                }
                                IconButton(onClick = { editingButtonIndex = index }) {
                                    Icon(
                                        Icons.Default.Edit,
                                        contentDescription = "Edit Button $index Action",
                                        tint = MaterialTheme.colorScheme.primary
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Modal Dialog to configure express key shortcut
    editingButtonIndex?.let { buttonIndex ->
        val currentAction = when (buttonIndex) {
            1 -> settings.button1Action
            2 -> settings.button2Action
            3 -> settings.button3Action
            4 -> settings.button4Action
            else -> ""
        }

        ShortcutConfigDialog(
            buttonNumber = buttonIndex,
            currentAction = currentAction,
            onDismiss = { editingButtonIndex = null },
            onSave = { newAction ->
                val updated = when (buttonIndex) {
                    1 -> settings.copy(button1Action = newAction)
                    2 -> settings.copy(button2Action = newAction)
                    3 -> settings.copy(button3Action = newAction)
                    4 -> settings.copy(button4Action = newAction)
                    else -> settings
                }
                onSettingsChange(updated)
                editingButtonIndex = null
            }
        )
    }
}

@Composable
fun ShortcutConfigDialog(
    buttonNumber: Int,
    currentAction: String,
    onDismiss: () -> Unit,
    onSave: (String) -> Unit
) {
    var selectedPreset by remember { mutableStateOf(if (currentAction in PRESET_SHORTCUTS) currentAction else "Custom") }
    var customText by remember { mutableStateOf(if (currentAction !in PRESET_SHORTCUTS) currentAction else "") }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Configure Button $buttonNumber Shortcut") },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    "Choose a common shortcut action or enter a custom key combination:",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                PRESET_SHORTCUTS.forEach { preset ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        RadioButton(
                            selected = selectedPreset == preset,
                            onClick = { selectedPreset = preset }
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(preset, style = MaterialTheme.typography.bodyMedium)
                    }
                }

                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    RadioButton(
                        selected = selectedPreset == "Custom",
                        onClick = { selectedPreset = "Custom" }
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Custom Action…", style = MaterialTheme.typography.bodyMedium)
                }

                if (selectedPreset == "Custom") {
                    OutlinedTextField(
                        value = customText,
                        onValueChange = { customText = it },
                        label = { Text("Custom Key / Shortcut") },
                        placeholder = { Text("e.g. Ctrl+Alt+T") },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 4.dp),
                        singleLine = true
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    val finalAction = if (selectedPreset == "Custom") {
                        customText.trim().ifEmpty { "None" }
                    } else {
                        selectedPreset
                    }
                    onSave(finalAction)
                }
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        }
    )
}
