package com.codex.mobile

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
private fun SettingsPanel(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
    content: @Composable ColumnScope.() -> Unit
) {
    Surface(shape = RoundedCornerShape(21.dp),
        color = MaterialTheme.colorScheme.surface, tonalElevation = 1.dp) {
        Column(Modifier.fillMaxWidth().padding(17.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Surface(shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.primary.copy(alpha = .09f)) {
                    Icon(icon, null, tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(10.dp).size(22.dp))
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(title, fontWeight = FontWeight.SemiBold)
                    Text(subtitle, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(15.dp))
            content()
        }
    }
}

@Composable
internal fun SettingsScreen(state: MobileState) {
    var key by remember { mutableStateOf("") }
    val clipboard = LocalClipboardManager.current
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())
        .padding(horizontal = 18.dp)) {
        Spacer(Modifier.height(19.dp))
        Text("设置", fontSize = 26.sp, fontWeight = FontWeight.SemiBold)
        Text("账号、安全、外观与运行状态",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.bodySmall)
        Spacer(Modifier.height(17.dp))
        Surface(shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.primary.copy(alpha = .1f)) {
            Row(Modifier.fillMaxWidth().padding(19.dp),
                verticalAlignment = Alignment.CenterVertically) {
                Icon(if (state.connected) Icons.Outlined.CheckCircle
                    else Icons.Outlined.SyncProblem, null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(29.dp))
                Spacer(Modifier.width(13.dp))
                Column(Modifier.weight(1f)) {
                    Text(if (state.connected) "原生核心已连接" else "正在连接 Codex",
                        fontWeight = FontWeight.SemiBold)
                    Text(state.status, style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = { state.restartCore() }) {
                    Icon(Icons.Outlined.Refresh, "重新连接")
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        SettingsPanel(Icons.Outlined.AccountCircle, "ChatGPT 账号",
            state.accountDescription) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = state::loginChatGpt, enabled = state.connected,
                    modifier = Modifier.weight(1f)) { Text("浏览器授权登录") }
                Spacer(Modifier.width(7.dp))
                OutlinedButton(onClick = state::readAccount, enabled = state.connected) {
                    Icon(Icons.Outlined.Refresh, "刷新账号", Modifier.size(20.dp))
                }
            }
            if (state.loginCode.isNotBlank()) {
                Spacer(Modifier.height(12.dp))
                Surface(shape = RoundedCornerShape(15.dp),
                    color = MaterialTheme.colorScheme.surfaceContainer) {
                    Row(Modifier.fillMaxWidth().padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("设备授权码", style = MaterialTheme.typography.labelSmall)
                            Text(state.loginCode, fontFamily = FontFamily.Monospace,
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.SemiBold)
                        }
                        IconButton(onClick = {
                            clipboard.setText(AnnotatedString(state.loginCode))
                        }) { Icon(Icons.Outlined.ContentCopy, "复制授权码") }
                    }
                }
                Spacer(Modifier.height(5.dp))
                Text("授权后会自动同步账号状态。",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            TextButton(onClick = state::logout, enabled = state.connected) {
                Text("退出账号")
            }
        }
        Spacer(Modifier.height(11.dp))
        SettingsPanel(Icons.Outlined.Key, "模型 API 密钥",
            "使用 Android Keystore 加密，凭据仅留在手机上") {
            OutlinedTextField(value = key, onValueChange = { key = it },
                modifier = Modifier.fillMaxWidth(), singleLine = true,
                placeholder = { Text("输入 API Key") },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password))
            Spacer(Modifier.height(10.dp))
            Button(onClick = { state.saveKey(key); key = "" },
                enabled = key.isNotBlank(), modifier = Modifier.fillMaxWidth()) {
                Text("加密保存并重新连接")
            }
            TextButton(onClick = { state.saveKey(""); key = "" }) {
                Text("清除已保存的密钥")
            }
        }
        Spacer(Modifier.height(11.dp))
        SettingsPanel(Icons.Outlined.Palette, "外观", "适应明暗环境与个人习惯") {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("深色模式")
                    Text("更适合夜间操作", style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Switch(checked = state.darkMode,
                    onCheckedChange = { state.darkMode = it })
            }
        }
        Spacer(Modifier.height(11.dp))
        SettingsPanel(Icons.Outlined.Terminal, "本地运行环境",
            "ARM64 官方 Codex App Server") {
            Text("工作区位于应用私有存储，工具执行受 Android 平台限制。",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(10.dp))
            OutlinedButton(onClick = { state.restartCore() },
                modifier = Modifier.fillMaxWidth()) {
                Icon(Icons.Outlined.Refresh, null, Modifier.size(18.dp))
                Spacer(Modifier.width(7.dp))
                Text("重启本地核心")
            }
        }
        Spacer(Modifier.height(20.dp))
        Text("Codex Mobile 0.3.0 · 原生 App Server",
            modifier = Modifier.fillMaxWidth(),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(32.dp))
    }
}
