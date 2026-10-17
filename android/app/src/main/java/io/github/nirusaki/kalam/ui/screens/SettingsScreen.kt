package io.github.nirusaki.kalam.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    isConnected: Boolean,
    activeButton: Int,
    settings: TabletSettings,
    onSettingsChange: (TabletSettings) -> Unit,
    onRefreshUsb: () -> Unit,
    modifier: Modifier = Modifier
) {
    val scrollState = rememberScrollState()

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
                        if (isConnected) "Connected via USB OTG (VID: 0x256C)" else "Disconnected · Plug in tablet OTG",
                        style = MaterialTheme.typography.bodySmall,
                        color = if (isConnected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error
                    )
                }
                FilledTonalButton(onClick = onRefreshUsb) {
                    Text("Detect")
                }
            }
        }

        // Hardware layout with live express-key feedback
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text("Tablet Hardware Reference", style = MaterialTheme.typography.titleSmall)
                Text(
                    "Cable at top. Buttons 1-2 on left, 3-4 on right. Press hardware buttons to test.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(8.dp))
                TabletDiagram(activeButton = activeButton)
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

        // Express Buttons Mapping Card
        OutlinedCard(modifier = Modifier.fillMaxWidth()) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Configured Express Keys", style = MaterialTheme.typography.titleSmall)
                listOf(
                    "Button 1 (Upper Left)" to settings.button1Action,
                    "Button 2 (Lower Left)" to settings.button2Action,
                    "Button 3 (Upper Right)" to settings.button3Action,
                    "Button 4 (Lower Right)" to settings.button4Action
                ).forEach { (btn, act) ->
                    ListItem(
                        headlineContent = { Text(btn) },
                        supportingContent = { Text(act) }
                    )
                }
            }
        }
    }
}
