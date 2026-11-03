package com.codex.mobile

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONArray
import org.json.JSONObject

private fun groupDescription(key: String) = when (key) {
    "thread" -> "新建、恢复、搜索与管理会话"
    "turn" -> "任务执行、停止与交互"
    "fs" -> "文件读写、复制、目录与监听"
    "command" -> "运行命令、流式输出与终端"
    "process" -> "管理持续运行的进程"
    "mcpServer" -> "连接工具服务和授权"
    "plugin" -> "浏览、安装与管理插件"
    "skills" -> "发现和配置 Codex 技能"
    "project" -> "浏览及整理项目与仓库"
    "review" -> "分析代码修改和质量"
    "config" -> "运行配置与服务设置"
    "account" -> "登录、用量与授权"
    else -> "访问完整 Codex 原生接口"
}

private fun groupIcon(key: String) = when (key) {
    "thread" -> Icons.Outlined.ChatBubbleOutline
    "turn" -> Icons.Outlined.PlayCircleOutline
    "fs" -> Icons.Outlined.FolderOpen
    "command" -> Icons.Outlined.Terminal
    "process" -> Icons.Outlined.Memory
    "mcpServer" -> Icons.Outlined.Hub
    "plugin" -> Icons.Outlined.Extension
    "skills" -> Icons.Outlined.AutoAwesome
    "project" -> Icons.Outlined.AccountTree
    "review" -> Icons.Outlined.FactCheck
    "config" -> Icons.Outlined.Tune
    "account" -> Icons.Outlined.AccountCircle
    else -> Icons.Outlined.GridView
}

private fun methodName(method: String) = when (method) {
    "command/exec" -> "运行终端命令"
    "skills/list" -> "查看已加载技能"
    "plugin/list" -> "浏览插件目录"
    "plugin/installed" -> "已安装插件"
    "config/read" -> "读取当前配置"
    "account/read" -> "查看账号状态"
    "thread/list" -> "查看会话"
    "fs/readFile" -> "读取工作区文件"
    "fs/writeFile" -> "写入文件"
    "fs/createDirectory" -> "新建文件夹"
    "fs/remove" -> "删除文件"
    "model/list" -> "列出模型"
    "app/list" -> "连接器列表"
    "review/start" -> "发起代码审查"
    else -> method.substringAfterLast("/")
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CapabilitiesScreen(state: MobileState) {
    val groups = state.featureGroups.sortedBy {
        listOf("thread", "turn", "fs", "command", "project", "review", "skills", "plugin",
            "mcpServer", "app", "account", "config").indexOf(it.key).let { position ->
            if (position == -1) 100 else position
        }
    }
    var query by remember { mutableStateOf("") }
    var currentGroup by remember { mutableStateOf<FeatureGroup?>(null) }
    var openMethod by remember { mutableStateOf<String?>(null) }
    var terminalDialog by remember { mutableStateOf(false) }
    var command by remember { mutableStateOf("") }
    val found = groups.filter { group ->
        group.title.contains(query, ignoreCase = true) ||
            group.methods.any { it.contains(query, ignoreCase = true) }
    }
    val methods = currentGroup?.methods?.filter {
        query.isBlank() || it.contains(query, ignoreCase = true) ||
            methodName(it).contains(query, ignoreCase = true)
    } ?: emptyList()

    Column(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 17.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (currentGroup != null) {
                    IconButton(onClick = { currentGroup = null; query = "" }) {
                        Icon(Icons.Outlined.ArrowBack, "返回功能总览")
                    }
                    Spacer(Modifier.width(4.dp))
                }
                Column(Modifier.weight(1f)) {
                    Text(currentGroup?.title ?: "功能中心", fontSize = 25.sp,
                        fontWeight = FontWeight.SemiBold)
                    Text(currentGroup?.let { groupDescription(it.key) }
                        ?: "按任务快速访问 Codex 的原生能力",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall)
                }
                Surface(shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.primary.copy(alpha = 0.09f)) {
                    Text(if (currentGroup != null) currentGroup!!.methods.size.toString()
                        else state.featureGroups.sumOf { it.methods.size }.toString(),
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                        fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.primary)
                }
            }
            Spacer(Modifier.height(17.dp))
            OutlinedTextField(
                value = query, onValueChange = { query = it },
                leadingIcon = { Icon(Icons.Outlined.Search, null) },
                trailingIcon = {
                    if (query.isNotEmpty()) IconButton(onClick = { query = "" }) {
                        Icon(Icons.Outlined.Close, "清除")
                    }
                },
                placeholder = { Text("搜索功能、插件、工具或接口") },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(17.dp), singleLine = true
            )
            Spacer(Modifier.height(14.dp))
        }
        if (currentGroup == null) {
            LazyColumn(Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 18.dp, end = 18.dp, bottom = 25.dp),
                verticalArrangement = Arrangement.spacedBy(9.dp)) {
                if (query.isBlank()) {
                    item {
                        Surface(shape = RoundedCornerShape(23.dp),
                            color = MaterialTheme.colorScheme.primary.copy(alpha = .07f)) {
                            Column(Modifier.padding(17.dp)) {
                                Text("常用任务", fontWeight = FontWeight.SemiBold,
                                    style = MaterialTheme.typography.titleMedium)
                                Spacer(Modifier.height(10.dp))
                                LazyRow(horizontalArrangement = Arrangement.spacedBy(9.dp)) {
                                    item {
                                        FilledTonalButton(onClick = { terminalDialog = true }) {
                                            Icon(Icons.Outlined.Terminal, null, Modifier.size(17.dp))
                                            Spacer(Modifier.width(5.dp))
                                            Text("终端")
                                        }
                                    }
                                    item {
                                        FilledTonalButton(onClick = {
                                            state.invokeQuickMethod("skills/list",
                                                JSONObject().put("cwds",
                                                    JSONArray().put(state.engine.workspace().absolutePath)))
                                            state.chooseMethod("skills", "skills/list")
                                            openMethod = "skills/list"
                                        }) { Text("我的技能") }
                                    }
                                    item {
                                        FilledTonalButton(onClick = {
                                            state.invokeQuickMethod("plugin/installed")
                                            state.chooseMethod("plugin", "plugin/installed")
                                            openMethod = "plugin/installed"
                                        }) { Text("插件") }
                                    }
                                    item {
                                        FilledTonalButton(onClick = {
                                            state.invokeQuickMethod("account/read")
                                            state.chooseMethod("account", "account/read")
                                            openMethod = "account/read"
                                        }) { Text("账号状态") }
                                    }
                                }
                            }
                        }
                    }
                    item {
                        Text("浏览全部分类", fontWeight = FontWeight.SemiBold,
                            style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.padding(top = 12.dp, bottom = 5.dp, start = 3.dp))
                    }
                }
                items(found, key = { it.key }) { group ->
                    Surface(shape = RoundedCornerShape(19.dp),
                        color = MaterialTheme.colorScheme.surface, tonalElevation = 1.dp,
                        modifier = Modifier.fillMaxWidth().clickable {
                            currentGroup = group
                            if (query.isNotBlank() && group.methods.none {
                                it.contains(query, ignoreCase = true)
                            }) query = ""
                        }) {
                        Row(Modifier.padding(15.dp), verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.size(47.dp),
                                contentAlignment = Alignment.Center) {
                                Surface(shape = RoundedCornerShape(15.dp),
                                    color = MaterialTheme.colorScheme.primary.copy(alpha = .09f)) {
                                    Icon(groupIcon(group.key), null,
                                        tint = MaterialTheme.colorScheme.primary,
                                        modifier = Modifier.padding(11.dp).size(23.dp))
                                }
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(group.title, fontWeight = FontWeight.SemiBold)
                                Spacer(Modifier.height(3.dp))
                                Text(groupDescription(group.key),
                                    maxLines = 2, overflow = TextOverflow.Ellipsis,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Text(group.methods.size.toString(),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Spacer(Modifier.width(6.dp))
                            Icon(Icons.Outlined.ChevronRight, null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        } else {
            LazyColumn(Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = 18.dp, vertical = 6.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(methods, key = { it }) { method ->
                    Surface(shape = RoundedCornerShape(16.dp),
                        color = MaterialTheme.colorScheme.surface, tonalElevation = 1.dp,
                        modifier = Modifier.fillMaxWidth().clickable {
                            state.chooseMethod(currentGroup!!.key, method)
                            openMethod = method
                        }) {
                        Row(Modifier.padding(15.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(methodName(method), fontWeight = FontWeight.Medium)
                                Text(method, style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Icon(Icons.Outlined.ChevronRight, null,
                                tint = MaterialTheme.colorScheme.primary)
                        }
                    }
                }
                item { Spacer(Modifier.height(22.dp)) }
            }
        }
    }

    openMethod?.let { method ->
        var params by remember(method) { mutableStateOf("{}") }
        var rawMode by remember(method) { mutableStateOf(false) }
        ModalBottomSheet(onDismissRequest = { openMethod = null }) {
            Column(Modifier.fillMaxWidth().padding(horizontal = 22.dp)
                .verticalScroll(rememberScrollState())) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(groupIcon(method.substringBefore('/')), null,
                        tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(25.dp))
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(methodName(method), style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold)
                        Text(method, style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.height(14.dp))
                Text("参数和执行结果与本地 Codex 核心直接交互。",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(12.dp))
                TextButton(onClick = { rawMode = !rawMode }) {
                    Icon(Icons.Outlined.Tune, null, Modifier.size(17.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(if (rawMode) "使用智能表单" else "编辑原始 JSON")
                }
                if (rawMode) {
                    OutlinedTextField(value = params, onValueChange = { params = it },
                        modifier = Modifier.fillMaxWidth(), minLines = 4, maxLines = 14,
                        textStyle = MaterialTheme.typography.bodySmall.copy(
                            fontFamily = FontFamily.Monospace),
                        label = { Text("请求参数") })
                } else {
                    RpcSchemaForm(state, method) { params = it }
                }
                Spacer(Modifier.height(15.dp))
                Button(
                    onClick = { state.invokeMethod(method, params) },
                    enabled = state.connected && !state.operationRunning,
                    modifier = Modifier.fillMaxWidth().height(49.dp)
                ) {
                    if (state.operationRunning) {
                        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(7.dp))
                    } else Icon(Icons.Outlined.PlayArrow, null, Modifier.size(20.dp))
                    Spacer(Modifier.width(7.dp))
                    Text(if (state.operationRunning) "正在执行" else "运行功能")
                }
                if (state.responseJson.isNotBlank() && state.lastOperationMethod == method) {
                    Spacer(Modifier.height(15.dp))
                    Text("执行结果", fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.height(7.dp))
                    Surface(shape = RoundedCornerShape(17.dp),
                        color = MaterialTheme.colorScheme.surfaceContainer) {
                        SelectionContainer {
                            Text(state.responseJson,
                                modifier = Modifier.fillMaxWidth().padding(14.dp),
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace))
                        }
                    }
                }
                Spacer(Modifier.height(35.dp))
            }
        }
    }

    if (terminalDialog) {
        AlertDialog(
            onDismissRequest = { terminalDialog = false },
            icon = { Icon(Icons.Outlined.Terminal, null) },
            title = { Text("工作区终端") },
            text = {
                Column {
                    Text("在手机工作区运行系统可用的命令。",
                        style = MaterialTheme.typography.bodySmall)
                    Spacer(Modifier.height(12.dp))
                    OutlinedTextField(value = command, onValueChange = { command = it },
                        label = { Text("命令") }, placeholder = { Text("ls -la") },
                        modifier = Modifier.fillMaxWidth(), minLines = 2, maxLines = 5,
                        textStyle = MaterialTheme.typography.bodyMedium.copy(
                            fontFamily = FontFamily.Monospace))
                }
            },
            confirmButton = {
                TextButton(enabled = command.isNotBlank() && state.connected, onClick = {
                    val params = JSONObject().put("command",
                        JSONArray().put("/system/bin/sh").put("-c").put(command))
                        .put("cwd", state.engine.workspace().absolutePath)
                    state.chooseMethod("command", "command/exec")
                    state.invokeMethod("command/exec", params.toString())
                    openMethod = "command/exec"
                    terminalDialog = false
                }) { Text("运行") }
            },
            dismissButton = { TextButton(onClick = { terminalDialog = false }) { Text("取消") } }
        )
    }
}
