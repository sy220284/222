package com.codex.mobile

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private fun eventTitle(method: String): String = when {
    method.startsWith("item/commandExecution") || method.startsWith("command/") -> "终端执行"
    method.startsWith("item/fileChange") || method.startsWith("fs/") -> "文件修改"
    method.startsWith("item/mcp") || method.startsWith("mcpServer") -> "外部工具"
    method.startsWith("turn/plan") -> "执行计划"
    method.startsWith("turn/diff") -> "代码差异"
    method.startsWith("item/started") -> "开始操作"
    method.startsWith("item/completed") -> "操作完成"
    method.startsWith("thread/") -> "会话更新"
    method.startsWith("turn/") -> "任务进度"
    else -> "运行事件"
}

private fun eventCategory(method: String): String = when {
    method.startsWith("command/") || method.contains("commandExecution") -> "命令"
    method.startsWith("fs/") || method.contains("fileChange") -> "文件"
    method.contains("mcp") || method.contains("tool") -> "工具"
    else -> "其他"
}

@Composable
internal fun ActivityScreen(state: MobileState) {
    var category by remember { mutableStateOf("全部") }
    var expanded by remember { mutableStateOf<String?>(null) }
    val list = remember(state.activities.size, category) {
        state.activities.filter {
            category == "全部" || eventCategory(it.text.substringBefore(" · ")) == category
        }
    }
    val scroll = rememberLazyListState()
    LaunchedEffect(state.activities.size, category) {
        if (list.isNotEmpty()) scroll.animateScrollToItem(list.lastIndex)
    }

    Column(Modifier.fillMaxSize()) {
        Column(Modifier.padding(start = 20.dp, end = 20.dp, top = 20.dp)) {
            Text("执行过程", fontSize = 25.sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.height(5.dp))
            Text("每一步命令、文件修改和工具调用都有记录",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodySmall)
            Spacer(Modifier.height(16.dp))
            Surface(shape = RoundedCornerShape(19.dp),
                color = MaterialTheme.colorScheme.surface) {
                Row(Modifier.fillMaxWidth().padding(15.dp),
                    verticalAlignment = Alignment.CenterVertically) {
                    if (state.busy) CircularProgressIndicator(Modifier.size(21.dp),
                        strokeWidth = 2.dp)
                    else Icon(Icons.Outlined.CheckCircleOutline, null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(23.dp))
                    Spacer(Modifier.width(13.dp))
                    Column(Modifier.weight(1f)) {
                        Text(if (state.busy) "正在运行" else "当前状态",
                            fontWeight = FontWeight.Medium)
                        Text(state.status, style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (state.busy) TextButton(onClick = state::interrupt) {
                        Text("停止")
                    }
                }
            }
            Spacer(Modifier.height(14.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                listOf("全部", "命令", "文件", "工具").forEach { option ->
                    FilterChip(selected = category == option, onClick = { category = option },
                        label = { Text(option, fontSize = 12.sp) })
                }
            }
            Spacer(Modifier.height(7.dp))
            Text(list.size.toString() + " 条事件", style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
        }
        LazyColumn(state = scroll, modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(horizontal = 18.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(9.dp)) {
            if (list.isEmpty()) {
                item {
                    Column(Modifier.fillMaxWidth().padding(32.dp),
                        horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(Icons.Outlined.Timeline, null, modifier = Modifier.size(43.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.height(12.dp))
                        Text("执行记录将在这里实时出现")
                    }
                }
            }
            items(list, key = { it.id }) { record ->
                val method = record.text.substringBefore(" · ")
                val detail = record.text.substringAfter(" · ", "")
                val isExpanded = expanded == record.id
                Surface(shape = RoundedCornerShape(18.dp),
                    color = MaterialTheme.colorScheme.surface, tonalElevation = 1.dp,
                    modifier = Modifier.fillMaxWidth().clickable {
                        expanded = if (isExpanded) null else record.id
                    }) {
                    Column(Modifier.padding(15.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Surface(shape = RoundedCornerShape(12.dp),
                                color = MaterialTheme.colorScheme.primary.copy(alpha = .08f)) {
                                Icon(when (eventCategory(method)) {
                                    "命令" -> Icons.Outlined.Terminal
                                    "文件" -> Icons.Outlined.Description
                                    "工具" -> Icons.Outlined.Extension
                                    else -> Icons.Outlined.Bolt
                                }, null,
                                    tint = MaterialTheme.colorScheme.primary,
                                    modifier = Modifier.padding(10.dp).size(19.dp))
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(eventTitle(method), fontWeight = FontWeight.SemiBold)
                                Text(method, style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            Icon(if (isExpanded) Icons.Outlined.ExpandLess
                                else Icons.Outlined.ExpandMore, null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (detail.isNotBlank()) {
                            Spacer(Modifier.height(9.dp))
                            SelectionContainer {
                                Text(if (isExpanded) detail else detail.take(170),
                                    fontFamily = FontFamily.Monospace,
                                    style = MaterialTheme.typography.bodySmall,
                                    maxLines = if (isExpanded) Int.MAX_VALUE else 3,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
    }
}
