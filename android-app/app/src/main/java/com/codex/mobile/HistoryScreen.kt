package com.codex.mobile

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun HistoryScreen(state: MobileState, onOpen: () -> Unit) {
    var query by remember { mutableStateOf("") }
    var rename by remember { mutableStateOf<SessionLine?>(null) }
    var title by remember { mutableStateOf("") }
    var archive by remember { mutableStateOf<SessionLine?>(null) }
    val results = remember(query, state.sessions.size) {
        state.sessions.filter {
            it.title.contains(query, ignoreCase = true) ||
                it.id.contains(query, ignoreCase = true)
        }
    }
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.padding(horizontal = 20.dp, vertical = 18.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("会话记录", fontSize = 26.sp, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.height(6.dp))
                    Text("随时恢复你在手机上进行的工作",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall)
                }
                FilledTonalIconButton(onClick = state::refresh) {
                    Icon(Icons.Outlined.Refresh, "刷新会话")
                }
            }
            Spacer(Modifier.height(17.dp))
            OutlinedTextField(
                value = query, onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth(), singleLine = true,
                leadingIcon = { Icon(Icons.Outlined.Search, null) },
                placeholder = { Text("搜索名称或会话编号") },
                trailingIcon = {
                    if (query.isNotEmpty()) IconButton(onClick = { query = "" }) {
                        Icon(Icons.Outlined.Close, "清除搜索")
                    }
                },
                shape = RoundedCornerShape(17.dp)
            )
            Spacer(Modifier.height(12.dp))
            Text("已加载 " + state.sessions.size + " 条 · 匹配 " + results.size + " 条",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        LazyColumn(Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 18.dp, end = 18.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(9.dp)) {
            if (results.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(vertical = 38.dp),
                        horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(Icons.Outlined.History, null, modifier = Modifier.size(40.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(10.dp))
                        Text(if (state.sessions.isEmpty()) "暂无会话" else "没有匹配的会话",
                            style = MaterialTheme.typography.titleMedium)
                        Spacer(Modifier.height(9.dp))
                        Text("可以从对话页面开启新任务",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            items(results, key = { it.id }) { item ->
                Surface(shape = RoundedCornerShape(19.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Row(Modifier.weight(1f).clickable {
                            state.openThread(item.id)
                            onOpen()
                        }.padding(start = 14.dp, top = 15.dp, bottom = 15.dp),
                            verticalAlignment = Alignment.CenterVertically) {
                            Surface(shape = RoundedCornerShape(13.dp),
                                color = MaterialTheme.colorScheme.primary.copy(alpha = .09f)) {
                                Icon(Icons.Outlined.ChatBubbleOutline, null,
                                    tint = MaterialTheme.colorScheme.primary,
                                    modifier = Modifier.padding(12.dp).size(20.dp))
                            }
                            Spacer(Modifier.width(13.dp))
                            Column(Modifier.weight(1f)) {
                                Text(item.title, maxLines = 2, overflow = TextOverflow.Ellipsis,
                                    fontWeight = FontWeight.SemiBold)
                                Spacer(Modifier.height(4.dp))
                                Text(item.id.take(12),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                        var menu by remember { mutableStateOf(false) }
                        Box {
                            IconButton(onClick = { menu = true }) {
                                Icon(Icons.Outlined.MoreVert, "更多操作")
                            }
                            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                                DropdownMenuItem(text = { Text("打开会话") }, onClick = {
                                    menu = false
                                    state.openThread(item.id)
                                    onOpen()
                                })
                                DropdownMenuItem(text = { Text("重命名") }, onClick = {
                                    menu = false
                                    title = item.title
                                    rename = item
                                })
                                DropdownMenuItem(text = { Text("归档") }, onClick = {
                                    menu = false
                                    archive = item
                                })
                            }
                        }
                    }
                }
            }
            if (state.sessionHasMore && query.isEmpty()) {
                item {
                    OutlinedButton(
                        onClick = state::loadMoreSessions,
                        enabled = !state.sessionLoading,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        if (state.sessionLoading) {
                            CircularProgressIndicator(Modifier.size(17.dp), strokeWidth = 2.dp)
                            Spacer(Modifier.width(8.dp))
                        }
                        Text(if (state.sessionLoading) "加载中" else "加载更多会话")
                    }
                }
            }
        }
    }

    rename?.let { selected ->
        AlertDialog(onDismissRequest = { rename = null },
            title = { Text("重命名会话") },
            text = { OutlinedTextField(value = title, onValueChange = { title = it },
                modifier = Modifier.fillMaxWidth(), singleLine = true) },
            confirmButton = {
                TextButton(enabled = title.isNotBlank(), onClick = {
                    state.rename(selected.id, title.trim())
                    rename = null
                }) { Text("保存") }
            },
            dismissButton = { TextButton(onClick = { rename = null }) { Text("取消") } }
        )
    }
    archive?.let { selected ->
        AlertDialog(onDismissRequest = { archive = null },
            title = { Text("归档会话") },
            text = { Text("会话「" + selected.title + "」将从当前列表中移除，可在高级功能中查看归档。") },
            confirmButton = {
                TextButton(onClick = { state.archive(selected.id); archive = null }) {
                    Text("归档")
                }
            },
            dismissButton = { TextButton(onClick = { archive = null }) { Text("取消") } }
        )
    }
}
