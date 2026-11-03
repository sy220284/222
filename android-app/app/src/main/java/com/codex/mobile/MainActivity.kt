package com.codex.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.launch
import org.json.JSONObject

class MainActivity : ComponentActivity() {
    private lateinit var model: MobileState
    private val pickDocument = registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        for (uri in uris) model.importAttachment(uri)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        model = (application as CodexMobileApp).state
        setContent {
            CodexAppearance(dark = model.darkMode) {
                MobileShell(model) {
                    pickDocument.launch(arrayOf("image/*", "audio/*", "text/*", "application/*"))
                }
            }
        }
        model.start()
    }

    override fun onDestroy() {
        // The application-scoped runtime and foreground service own active tasks.
        super.onDestroy()
    }
}

private val Ink = Color(0xFF182336)
private val Deep = Color(0xFF0C1524)
private val LightBackground = Color(0xFFF5F7FB)
private val LightPanel = Color(0xFFFFFFFF)
private val Blue = Color(0xFF5265E8)
private val Muted = Color(0xFF69778D)

@Composable
private fun CodexAppearance(dark: Boolean, content: @Composable () -> Unit) {
    val colors = if (dark) darkColorScheme(
        primary = Color(0xFFA5B8FF),
        onPrimary = Deep,
        background = Deep,
        surface = Color(0xFF151E2D),
        surfaceContainer = Color(0xFF202B3B),
        onSurface = Color(0xFFEAF0FA),
        onSurfaceVariant = Color(0xFFA9B4C8),
        outlineVariant = Color(0xFF3B4658)
    ) else lightColorScheme(
        primary = Blue,
        background = LightBackground,
        surface = LightPanel,
        surfaceContainer = Color(0xFFEEF1F7),
        onSurface = Ink,
        onSurfaceVariant = Muted,
        outlineVariant = Color(0xFFE2E7F0)
    )
    MaterialTheme(colorScheme = colors, shapes = Shapes(
        extraSmall = RoundedCornerShape(9.dp),
        small = RoundedCornerShape(12.dp),
        medium = RoundedCornerShape(16.dp),
        large = RoundedCornerShape(22.dp),
        extraLarge = RoundedCornerShape(28.dp)
    ), content = content)
}

private enum class Page(val title: String) {
    CHAT("对话"), HISTORY("会话记录"), FILES("工作区"),
    TOOLS("全部功能"), TIMELINE("执行过程"), SETTINGS("设置")
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MobileShell(state: MobileState, pickFile: () -> Unit) {
    val scope = rememberCoroutineScope()
    val drawer = rememberDrawerState(DrawerValue.Closed)
    var page by remember { mutableStateOf(Page.CHAT) }
    val context = LocalContext.current
    val animated = remember(context) {
        android.provider.Settings.Global.getFloat(context.contentResolver,
            android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 1f) != 0f
    }
    // Navigation and content remain independently scrollable.
    val pages = listOf(
        Page.CHAT to Icons.Outlined.ChatBubbleOutline,
        Page.HISTORY to Icons.Outlined.History,
        Page.FILES to Icons.Outlined.FolderOpen,
        Page.TOOLS to Icons.Outlined.GridView,
        Page.TIMELINE to Icons.Outlined.Timeline,
        Page.SETTINGS to Icons.Outlined.Settings
    )
    ModalNavigationDrawer(
        drawerState = drawer,
        drawerContent = {
            ModalDrawerSheet(modifier = Modifier.width(298.dp)) {
                Spacer(Modifier.height(22.dp))
                Row(Modifier.fillMaxWidth().padding(horizontal = 22.dp),
                    verticalAlignment = Alignment.CenterVertically) {
                    BrandMark()
                    Spacer(Modifier.width(12.dp))
                    Column {
                        Text("Codex", fontSize = 22.sp, fontWeight = FontWeight.Bold)
                        Text("Mobile Workspace", style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.height(30.dp))
                NavigationDrawerItem(
                    label = { Text("开启新任务") },
                    selected = false,
                    icon = { Icon(Icons.Outlined.Add, null) },
                    onClick = { state.newChat(); page = Page.CHAT; scope.launch { drawer.close() } },
                    modifier = Modifier.padding(horizontal = 12.dp)
                )
                Spacer(Modifier.height(14.dp))
                HorizontalDivider(Modifier.padding(horizontal = 24.dp))
                Spacer(Modifier.height(12.dp))
                pages.forEach { (destination, icon) ->
                    NavigationDrawerItem(
                        selected = page == destination,
                        onClick = {
                            page = destination
                            if (destination == Page.HISTORY) state.refresh()
                            scope.launch { drawer.close() }
                        },
                        icon = { Icon(icon, null) },
                        label = { Text(destination.title) },
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 2.dp)
                    )
                }
                Spacer(Modifier.weight(1f))
                Row(Modifier.padding(22.dp), verticalAlignment = Alignment.CenterVertically) {
                    ConnectionDot(state.connected)
                    Spacer(Modifier.width(10.dp))
                    Text(state.status, maxLines = 2,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    ) {
        Scaffold(
            containerColor = MaterialTheme.colorScheme.background,
            topBar = {
                TopAppBar(
                    title = {
                        Column {
                            Text(if (page == Page.CHAT) "工作台" else page.title,
                                fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                            if (page == Page.CHAT) Text(
                                if (state.connected) "已连接 · 原生 Codex" else "原生核心尚未连接",
                                fontSize = 11.sp,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = { scope.launch { drawer.open() } }) {
                            Icon(Icons.Outlined.Menu, contentDescription = "打开侧边栏")
                        }
                    },
                    actions = {
                        if (page == Page.CHAT) ModelPicker(state)
                        IconButton(onClick = { state.darkMode = !state.darkMode }) {
                            Icon(if (state.darkMode) Icons.Outlined.LightMode else Icons.Outlined.DarkMode,
                                contentDescription = "切换深色或浅色外观")
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.background)
                )
            },
            bottomBar = {
                Column {
                    if (page == Page.CHAT) Composer(state, pickFile)
                    NavigationBar(
                        tonalElevation = 0.dp,
                        containerColor = MaterialTheme.colorScheme.surface,
                        modifier = Modifier.fillMaxWidth().height(68.dp)
                    ) {
                        val primary = listOf(
                            Triple(Page.CHAT, Icons.Outlined.ChatBubbleOutline, "对话"),
                            Triple(Page.FILES, Icons.Outlined.FolderOpen, "文件"),
                            Triple(Page.TOOLS, Icons.Outlined.GridView, "功能"),
                            Triple(Page.HISTORY, Icons.Outlined.History, "历史")
                        )
                        primary.forEach { (destination, icon, title) ->
                            NavigationBarItem(
                                selected = page == destination,
                                onClick = {
                                    page = destination
                                    if (destination == Page.HISTORY) state.refresh()
                                },
                                icon = { Icon(icon, contentDescription = title, modifier = Modifier.size(22.dp)) },
                                label = { Text(title, fontSize = 11.sp) },
                                alwaysShowLabel = true,
                                colors = NavigationBarItemDefaults.colors(
                                    indicatorColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)
                                )
                            )
                        }
                    }
                }
            }
        ) { padding ->
            val duration = if (animated) 180 else 0
            AnimatedContent(
                targetState = page,
                modifier = Modifier.fillMaxSize().padding(padding),
                transitionSpec = {
                    fadeIn(tween(duration)) togetherWith fadeOut(tween(duration))
                },
                label = "page-transition"
            ) { current ->
                when (current) {
                    Page.CHAT -> ConversationPage(state)
                    Page.HISTORY -> HistoryScreen(state) { page = Page.CHAT }
                    Page.FILES -> WorkspaceScreen(state, pickFile)
                    Page.TOOLS -> CapabilitiesScreen(state)
                    Page.TIMELINE -> ActivityScreen(state)
                    Page.SETTINGS -> SettingsScreen(state)
                }
            }
        }
    }

    state.pendingApproval?.let { request ->
        ApprovalDialog(request,
            onDecline = { state.resolveApproval(false) },
            onAccept = { state.resolveApproval(true, it) })
    }
    state.lastError?.let { detail ->
        AlertDialog(
            onDismissRequest = { state.lastError = null },
            icon = { Icon(Icons.Outlined.ErrorOutline, null) },
            title = { Text("操作未完成") },
            text = { SelectionContainer { Text(detail) } },
            confirmButton = { TextButton(onClick = { state.lastError = null }) { Text("知道了") } }
        )
    }
}

@Composable
private fun BrandMark() {
    Box(Modifier.size(38.dp).clip(RoundedCornerShape(13.dp))
        .background(Brush.linearGradient(listOf(Blue, Color(0xFF7B76E4)))),
        contentAlignment = Alignment.Center) {
        Icon(Icons.Outlined.Code, null, tint = Color.White, modifier = Modifier.size(23.dp))
    }
}

@Composable
private fun ConnectionDot(connected: Boolean) {
    Box(Modifier.size(8.dp).clip(CircleShape).background(
        if (connected) Color(0xFF4CB69A) else Color(0xFFE6A25D)))
}

@Composable
private fun ModelPicker(state: MobileState) {
    var menu by remember { mutableStateOf(false) }
    Box {
        TextButton(onClick = {
            if (state.models.isEmpty()) state.getModels()
            menu = true
        }) {
            Text(state.selectedModel.ifBlank { "默认模型" }, maxLines = 1,
                style = MaterialTheme.typography.labelMedium)
            Spacer(Modifier.width(4.dp))
            Icon(Icons.Outlined.ExpandMore, null, modifier = Modifier.size(16.dp))
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text("自动选择") }, onClick = {
                state.selectedModel = ""
                menu = false
            })
            state.models.forEach { model ->
                DropdownMenuItem(text = { Text(model) }, onClick = {
                    state.selectedModel = model
                    menu = false
                })
            }
        }
    }
}

@Composable
private fun ConversationPage(state: MobileState) {
    if (state.chat.isEmpty()) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())
            .padding(horizontal = 26.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.Center) {
            Box(Modifier.size(66.dp).clip(RoundedCornerShape(22.dp))
                .background(MaterialTheme.colorScheme.surfaceContainer),
                contentAlignment = Alignment.Center) {
                Icon(Icons.Outlined.AutoAwesome, null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(30.dp))
            }
            Spacer(Modifier.height(22.dp))
            Text("从一个想法开始。", fontSize = 31.sp, lineHeight = 39.sp,
                letterSpacing = (-0.9).sp, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.height(10.dp))
            Text("理解代码、修复问题、审查改动，\n从这里直接运行 Codex。",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant, lineHeight = 26.sp)
            Spacer(Modifier.height(38.dp))
            val suggestions = listOf(
                "理解当前代码结构" to "分析当前工作区的项目结构，概括主要模块及调用关系",
                "审查代码质量" to "检查工作区代码，指出明显问题及可验证的修复建议",
                "协助实现功能" to "分析当前项目并给出完善指定功能所需的代码修改"
            )
            suggestions.forEach { (name, prompt) ->
                Surface(
                    modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp)
                        .clickable(enabled = state.connected) { state.send(prompt) },
                    shape = RoundedCornerShape(18.dp),
                    color = MaterialTheme.colorScheme.surface,
                    tonalElevation = 1.dp
                ) {
                    Row(Modifier.padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(name, modifier = Modifier.weight(1f), fontWeight = FontWeight.Medium)
                        Icon(Icons.Outlined.ArrowOutward, null,
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(18.dp))
                    }
                }
            }
            if (!state.connected) Text(state.status, modifier = Modifier.padding(top = 16.dp),
                color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
    } else {
        val listState = rememberLazyListState()
        val lastMessageSize = state.chat.lastOrNull()?.text?.length ?: 0
        LaunchedEffect(state.chat.size, lastMessageSize / 160) {
            if (state.chat.isNotEmpty()) listState.scrollToItem(state.chat.lastIndex)
        }
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(18.dp)
        ) {
            items(state.chat, key = { it.id }) { line -> ConversationMessage(line) }
            if (state.busy) item {
                Row(Modifier.fillMaxWidth().padding(start = 4.dp),
                    verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(Modifier.size(15.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(12.dp))
                    Text("正在处理请求…", color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }
}

@Composable
private fun ConversationMessage(line: ChatLine) {
    val user = line.role == "user"
    Column(Modifier.fillMaxWidth(),
        horizontalAlignment = if (user) Alignment.End else Alignment.Start) {
        Text(if (user) "你" else "Codex",
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Surface(
            modifier = Modifier.fillMaxWidth(if (user) .9f else 1f).animateContentSize(),
            shape = RoundedCornerShape(
                topStart = 20.dp, topEnd = 20.dp,
                bottomEnd = if (user) 6.dp else 20.dp,
                bottomStart = if (user) 20.dp else 6.dp),
            color = if (user) MaterialTheme.colorScheme.surfaceContainer
                else MaterialTheme.colorScheme.surface
        ) {
            MessageBody(line.text)
        }
    }
}

@Composable
private fun MessageBody(message: String) {
    val clipboard = androidx.compose.ui.platform.LocalClipboardManager.current
    val fragments = remember(message) { message.split(Char(96).toString().repeat(3)) }
    Column(Modifier.padding(horizontal = 16.dp, vertical = 13.dp),
        verticalArrangement = Arrangement.spacedBy(9.dp)) {
        fragments.forEachIndexed { index, segment ->
            if (index % 2 == 1) {
                val raw = segment.trim('\n')
                val label = raw.substringBefore('\n').trim()
                val language = if (label.length <= 18 &&
                    label.matches(Regex("[a-zA-Z0-9+#.\\-]*"))) label else ""
                val code = if (language.isNotEmpty()) raw.substringAfter('\n', "") else raw
                Surface(shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.surfaceContainer) {
                    Column {
                        Row(Modifier.fillMaxWidth().padding(start = 12.dp, top = 7.dp,
                            end = 5.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(language.ifBlank { "代码" }, modifier = Modifier.weight(1f),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant)
                            IconButton(onClick = {
                                clipboard.setText(androidx.compose.ui.text.AnnotatedString(code))
                            }, modifier = Modifier.size(30.dp)) {
                                Icon(Icons.Outlined.ContentCopy, "复制代码",
                                    modifier = Modifier.size(16.dp))
                            }
                        }
                        SelectionContainer {
                            Text(code, Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp,
                                bottom = 13.dp),
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace),
                                lineHeight = 19.sp)
                        }
                    }
                }
            } else if (segment.isNotBlank()) {
                SelectionContainer {
                    Text(segment.trim('\n'),
                        style = MaterialTheme.typography.bodyMedium, lineHeight = 23.sp)
                }
            }
        }
    }
}

@Composable
private fun Composer(state: MobileState, pickFile: () -> Unit) {
    var draft by remember(state.threadId) { mutableStateOf("") }
    Surface(
        shadowElevation = 8.dp,
        color = MaterialTheme.colorScheme.surface
    ) {
        Column(Modifier.fillMaxWidth().navigationBarsPadding().imePadding()
            .padding(horizontal = 14.dp, vertical = 10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp)) {
                FilterChip(
                    selected = state.writeAccess,
                    onClick = { if (state.threadId == null) state.writeAccess = !state.writeAccess
                        else state.lastError = "权限模式请在新会话开始前选择；当前会话可在全部功能中调整"
                    },
                    label = { Text(if (state.writeAccess) "允许修改工作区" else "只读模式",
                        fontSize = 12.sp) },
                    leadingIcon = { Icon(Icons.Outlined.Shield, null,
                        modifier = Modifier.size(15.dp)) }
                )
                Spacer(Modifier.weight(1f))
                ConnectionDot(state.connected)
                Spacer(Modifier.width(7.dp))
                Text(if (state.connected) "已连接" else "未连接",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Surface(
                shape = RoundedCornerShape(22.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                color = MaterialTheme.colorScheme.surface
            ) {
                Column(Modifier.fillMaxWidth().padding(10.dp)) {
                    OutlinedTextField(
                        value = draft, onValueChange = { draft = it },
                        modifier = Modifier.fillMaxWidth(),
                        placeholder = { Text("描述任务，或提出一个问题…") },
                        minLines = 1, maxLines = 5,
                        colors = OutlinedTextFieldDefaults.colors(
                            unfocusedBorderColor = Color.Transparent,
                            focusedBorderColor = Color.Transparent
                        ),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Default)
                    )
                    AnimatedVisibility(state.attachments.isNotEmpty()) {
                        Column {
                            state.attachments.forEach { file ->
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Icon(Icons.Outlined.AttachFile, null,
                                        modifier = Modifier.size(16.dp))
                                    Spacer(Modifier.width(5.dp))
                                    Text(file.name, style = MaterialTheme.typography.labelSmall,
                                        maxLines = 1, modifier = Modifier.weight(1f))
                                    IconButton(onClick = { state.attachments.remove(file) }) {
                                        Icon(Icons.Outlined.Close, "移除附件")
                                    }
                                }
                            }
                        }
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        IconButton(onClick = pickFile) {
                            Icon(Icons.Outlined.AttachFile, "添加图片、音频或文件")
                        }
                        Text("手机本地工作区", style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f))
                        if (state.busy) {
                            FilledTonalIconButton(onClick = state::interrupt) {
                                Icon(Icons.Outlined.Stop, "停止执行")
                            }
                        } else {
                            FilledIconButton(onClick = {
                                val text = draft.trim()
                                if (text.isNotBlank()) { draft = ""; state.send(text) }
                            }, enabled = state.connected && draft.isNotBlank()) {
                                Icon(Icons.Outlined.ArrowUpward, "发送请求")
                            }
                        }
                    }
                }
            }
        }
    }
}

