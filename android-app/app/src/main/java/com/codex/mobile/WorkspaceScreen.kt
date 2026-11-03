package com.codex.mobile

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import java.io.File

@Composable
internal fun WorkspaceScreen(state: MobileState, importFile: () -> Unit) {
    val manager = state.fileManager
    var current by remember { mutableStateOf(manager.root()) }
    var entries by remember { mutableStateOf<List<File>>(emptyList()) }
    var selected by remember { mutableStateOf<File?>(null) }
    var nameMode by remember { mutableStateOf<String?>(null) }
    var nameValue by remember { mutableStateOf("") }
    var renameTarget by remember { mutableStateOf<File?>(null) }
    var deleting by remember { mutableStateOf<File?>(null) }
    var editing by remember { mutableStateOf<File?>(null) }
    var content by remember { mutableStateOf("") }

    LaunchedEffect(current, state.workspaceRevision) {
        try { entries = manager.list(current) }
        catch (error: Exception) {
            state.lastError = error.message ?: "读取工作区失败"
            current = manager.root()
        }
    }

    Column(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 16.dp)) {
            Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(25.dp))
                .background(Brush.linearGradient(listOf(Color(0xFF253C83), Color(0xFF5968E9))))
                .padding(21.dp)) {
                Column {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.FolderOpen, null, tint = Color.White,
                            modifier = Modifier.size(26.dp))
                        Spacer(Modifier.width(12.dp))
                        Text("本地工作区", color = Color.White, fontSize = 20.sp,
                            fontWeight = FontWeight.SemiBold)
                    }
                    Spacer(Modifier.height(10.dp))
                    Text("代码、文件、修改记录都留在手机上。",
                        color = Color.White.copy(alpha = .86f),
                        style = MaterialTheme.typography.bodySmall)
                    Spacer(Modifier.height(14.dp))
                    Text(manager.relative(current), color = Color.White, fontWeight = FontWeight.Medium)
                }
            }
            Spacer(Modifier.height(13.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                FilledTonalButton(onClick = importFile) {
                    Icon(Icons.Outlined.FileUpload, null, Modifier.size(17.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("导入")
                }
                Spacer(Modifier.width(6.dp))
                OutlinedButton(onClick = { nameMode = "文件夹"; nameValue = "" }) {
                    Icon(Icons.Outlined.CreateNewFolder, null, Modifier.size(17.dp))
                    Spacer(Modifier.width(5.dp))
                    Text("新建")
                }
                Spacer(Modifier.weight(1f))
                if (current != manager.root()) {
                    IconButton(onClick = { current = manager.parent(current) }) {
                        Icon(Icons.Outlined.ArrowBack, "返回上级")
                    }
                }
                IconButton(onClick = { state.workspaceChanged() }) {
                    Icon(Icons.Outlined.Refresh, "刷新文件列表")
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().padding(top = 12.dp)) {
                Text("文件与文件夹", fontWeight = FontWeight.SemiBold,
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.weight(1f))
                Text(entries.size.toString() + " 项",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        LazyColumn(Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 18.dp, end = 18.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (entries.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(25.dp),
                        horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(Icons.Outlined.FolderOpen, null, Modifier.size(40.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(12.dp))
                        Text("这里还是空的", fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(5.dp))
                        Text("导入文件，或创建一个新项目。",
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            items(entries, key = { it.absolutePath }) { file ->
                Surface(shape = RoundedCornerShape(17.dp),
                    color = MaterialTheme.colorScheme.surface, tonalElevation = 1.dp) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Row(Modifier.weight(1f).clickable {
                            if (file.isDirectory) current = file else selected = file
                        }.padding(start = 14.dp, top = 12.dp, bottom = 12.dp),
                            verticalAlignment = Alignment.CenterVertically) {
                            Box(Modifier.size(41.dp).clip(RoundedCornerShape(12.dp))
                                .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.09f)),
                                contentAlignment = Alignment.Center) {
                                Icon(if (file.isDirectory) Icons.Outlined.Folder
                                    else Icons.Outlined.Description, null,
                                    tint = MaterialTheme.colorScheme.primary)
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(file.name, maxLines = 2, overflow = TextOverflow.Ellipsis,
                                    fontWeight = FontWeight.Medium)
                                Text(if (file.isDirectory) "文件夹"
                                     else ((file.length() + 1023) / 1024).toString() + " KB",
                                     style = MaterialTheme.typography.labelSmall,
                                     color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        IconButton(onClick = { selected = file }) {
                            Icon(Icons.Outlined.MoreVert, "管理文件")
                        }
                    }
                }
            }
            item {
                TextButton(onClick = { nameMode = "文件"; nameValue = "" },
                    modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Outlined.NoteAdd, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(5.dp))
                    Text("新建文本文件")
                }
            }
        }
    }

    selected?.let { file ->
        val preview = remember(file, state.workspaceRevision) {
            if (file.isDirectory) Result.success("这是一个文件夹") else runCatching { manager.readText(file) }
        }
        AlertDialog(
            onDismissRequest = { selected = null },
            title = { Text(file.name, maxLines = 2) },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    Text(manager.relative(file), style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.height(12.dp))
                    Surface(shape = RoundedCornerShape(14.dp),
                        color = MaterialTheme.colorScheme.surfaceContainer) {
                        SelectionContainer {
                            Text(preview.getOrElse { it.message ?: "无法预览" }.take(3000),
                                Modifier.fillMaxWidth().padding(12.dp), maxLines = 12,
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace))
                        }
                    }
                    Spacer(Modifier.height(10.dp))
                    if (file.isDirectory) {
                        TextButton(onClick = { current = file; selected = null }) {
                            Text("打开文件夹")
                        }
                    } else if (preview.isSuccess) {
                        TextButton(onClick = {
                            content = preview.getOrThrow()
                            editing = file
                            selected = null
                        }) { Text("编辑文件") }
                    }
                    TextButton(onClick = {
                        renameTarget = file
                        nameMode = "重命名"
                        nameValue = file.name
                        selected = null
                    }) { Text("重命名") }
                    TextButton(onClick = { deleting = file; selected = null },
                        colors = ButtonDefaults.textButtonColors(
                            contentColor = MaterialTheme.colorScheme.error)) {
                        Text("删除")
                    }
                }
            },
            confirmButton = { TextButton(onClick = { selected = null }) { Text("完成") } }
        )
    }

    nameMode?.let { mode ->
        AlertDialog(onDismissRequest = { nameMode = null },
            title = { Text(if (mode == "重命名") "重命名" else "新建" + mode) },
            text = { OutlinedTextField(value = nameValue, onValueChange = { nameValue = it },
                label = { Text("名称") }, singleLine = true) },
            confirmButton = {
                TextButton(enabled = nameValue.isNotBlank(), onClick = {
                    try {
                        when (mode) {
                            "文件夹" -> manager.createFolder(current, nameValue)
                            "文件" -> manager.createTextFile(current, nameValue)
                            "重命名" -> manager.rename(
                                requireNotNull(renameTarget) { "请选择要重命名的文件" }, nameValue)
                            else -> error("未知操作")
                        }
                        state.workspaceChanged()
                        nameMode = null
                        renameTarget = null
                    } catch (error: Exception) {
                        state.lastError = error.message ?: "文件操作失败"
                    }
                }) { Text("保存") }
            },
            dismissButton = { TextButton(onClick = { nameMode = null }) { Text("取消") } }
        )
    }

    deleting?.let { file ->
        AlertDialog(onDismissRequest = { deleting = null },
            title = { Text("确认删除？") },
            text = { Text("将永久删除「" + file.name + "」及其内部的文件。") },
            confirmButton = {
                TextButton(onClick = {
                    try {
                        manager.delete(file)
                        state.workspaceChanged()
                        deleting = null
                    } catch (error: Exception) {
                        state.lastError = error.message ?: "删除失败"
                    }
                }, colors = ButtonDefaults.textButtonColors(
                    contentColor = MaterialTheme.colorScheme.error)) { Text("删除") }
            },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("取消") } }
        )
    }

    editing?.let { file ->
        Dialog(onDismissRequest = { editing = null }) {
            Surface(shape = RoundedCornerShape(23.dp),
                modifier = Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.surface) {
                Column(Modifier.padding(18.dp)) {
                    Text("编辑 · " + file.name, style = MaterialTheme.typography.titleLarge,
                        maxLines = 2)
                    Spacer(Modifier.height(14.dp))
                    OutlinedTextField(value = content, onValueChange = { content = it },
                        modifier = Modifier.fillMaxWidth().heightIn(min = 180.dp, max = 400.dp),
                        textStyle = MaterialTheme.typography.bodySmall.copy(
                            fontFamily = FontFamily.Monospace), maxLines = 19)
                    Spacer(Modifier.height(12.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                        TextButton(onClick = { editing = null }) { Text("取消") }
                        Spacer(Modifier.width(8.dp))
                        Button(onClick = {
                            try {
                                manager.saveText(file, content)
                                state.workspaceChanged()
                                editing = null
                            } catch (error: Exception) {
                                state.lastError = error.message ?: "保存失败"
                            }
                        }) { Text("保存") }
                    }
                }
            }
        }
    }
}
