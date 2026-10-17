package io.github.nirusaki.kalam

import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import io.github.nirusaki.kalam.data.ConfigRepository
import io.github.nirusaki.kalam.data.UsbRepository
import io.github.nirusaki.kalam.model.TabletSettings
import io.github.nirusaki.kalam.ui.screens.SettingsScreen
import io.github.nirusaki.kalam.ui.screens.TestCanvasScreen
import io.github.nirusaki.kalam.ui.theme.KalamTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private lateinit var usbRepository: UsbRepository
    private lateinit var configRepository: ConfigRepository

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        usbRepository = UsbRepository(this)
        configRepository = ConfigRepository(this)

        setContent {
            KalamTheme {
                MainApp(usbRepository, configRepository)
            }
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (::usbRepository.isInitialized) {
            usbRepository.onHardwareKeyEvent(keyCode, true)
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent?): Boolean {
        if (::usbRepository.isInitialized) {
            usbRepository.onHardwareKeyEvent(keyCode, false)
        }
        return super.onKeyUp(keyCode, event)
    }

    override fun onDestroy() {
        super.onDestroy()
        if (::usbRepository.isInitialized) {
            usbRepository.release()
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainApp(
    usbRepository: UsbRepository,
    configRepository: ConfigRepository
) {
    var selectedTab by remember { mutableIntStateOf(0) }
    val scope = rememberCoroutineScope()

    val isConnected by usbRepository.isConnected.collectAsState()
    val connectionStatus by usbRepository.connectionStatus.collectAsState()
    val stylusReport by usbRepository.stylusReport.collectAsState()
    val keyReport by usbRepository.keyReport.collectAsState()

    val settings by configRepository.settingsFlow.collectAsState(initial = TabletSettings())

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Kalam Tablet") },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                )
            )
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    icon = { Icon(Icons.Default.Build, contentDescription = null) },
                    label = { Text("Settings") }
                )
                NavigationBarItem(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    icon = { Icon(Icons.Default.Edit, contentDescription = null) },
                    label = { Text("Test Pen") }
                )
            }
        }
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            if (selectedTab == 0) {
                SettingsScreen(
                    isConnected = isConnected,
                    connectionStatus = connectionStatus,
                    activeButton = keyReport.activeKeyIndex(),
                    settings = settings,
                    onSettingsChange = { updated ->
                        scope.launch {
                            configRepository.saveSettings(updated)
                        }
                    },
                    onRefreshUsb = { usbRepository.probeDevice() }
                )
            } else {
                TestCanvasScreen(
                    usbStylus = stylusReport,
                    keyReport = keyReport,
                    settings = settings
                )
            }
        }
    }
}
