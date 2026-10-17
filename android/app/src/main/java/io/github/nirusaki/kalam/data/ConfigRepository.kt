package io.github.nirusaki.kalam.data

import android.content.Context
import androidx.datastore.preferences.core.*
import androidx.datastore.preferences.preferencesDataStore
import io.github.nirusaki.kalam.model.CurvePreset
import io.github.nirusaki.kalam.model.OrientationPreset
import io.github.nirusaki.kalam.model.TabletSettings
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore by preferencesDataStore(name = "kalam_settings")

class ConfigRepository(private val context: Context) {
    private object PreferencesKeys {
        val PRESET_NAME = stringPreferencesKey("curve_preset")
        val P1_X = floatPreferencesKey("p1_x")
        val P1_Y = floatPreferencesKey("p1_y")
        val P2_X = floatPreferencesKey("p2_x")
        val P2_Y = floatPreferencesKey("p2_y")
        val ORIENTATION = intPreferencesKey("orientation_degrees")
        val BTN1 = stringPreferencesKey("btn1_action")
        val BTN2 = stringPreferencesKey("btn2_action")
        val BTN3 = stringPreferencesKey("btn3_action")
        val BTN4 = stringPreferencesKey("btn4_action")
    }

    val settingsFlow: Flow<TabletSettings> = context.dataStore.data.map { prefs ->
        val presetName = prefs[PreferencesKeys.PRESET_NAME] ?: CurvePreset.LINEAR.name
        val preset = try {
            CurvePreset.valueOf(presetName)
        } catch (_: Exception) {
            CurvePreset.LINEAR
        }
        val p1x = prefs[PreferencesKeys.P1_X] ?: preset.p1x
        val p1y = prefs[PreferencesKeys.P1_Y] ?: preset.p1y
        val p2x = prefs[PreferencesKeys.P2_X] ?: preset.p2x
        val p2y = prefs[PreferencesKeys.P2_Y] ?: preset.p2y
        val deg = prefs[PreferencesKeys.ORIENTATION] ?: 0
        val orientation = OrientationPreset.values().firstOrNull { it.degrees == deg }
            ?: OrientationPreset.NORMAL

        TabletSettings(
            preset = preset,
            customP1x = p1x,
            customP1y = p1y,
            customP2x = p2x,
            customP2y = p2y,
            orientation = orientation,
            button1Action = prefs[PreferencesKeys.BTN1] ?: "Undo · Ctrl+Z",
            button2Action = prefs[PreferencesKeys.BTN2] ?: "Redo · Ctrl+Shift+Z",
            button3Action = prefs[PreferencesKeys.BTN3] ?: "Brush · B",
            button4Action = prefs[PreferencesKeys.BTN4] ?: "Eraser · E"
        )
    }

    suspend fun saveSettings(settings: TabletSettings) {
        context.dataStore.edit { prefs ->
            prefs[PreferencesKeys.PRESET_NAME] = settings.preset.name
            prefs[PreferencesKeys.P1_X] = settings.customP1x
            prefs[PreferencesKeys.P1_Y] = settings.customP1y
            prefs[PreferencesKeys.P2_X] = settings.customP2x
            prefs[PreferencesKeys.P2_Y] = settings.customP2y
            prefs[PreferencesKeys.ORIENTATION] = settings.orientation.degrees
            prefs[PreferencesKeys.BTN1] = settings.button1Action
            prefs[PreferencesKeys.BTN2] = settings.button2Action
            prefs[PreferencesKeys.BTN3] = settings.button3Action
            prefs[PreferencesKeys.BTN4] = settings.button4Action
        }
    }
}
