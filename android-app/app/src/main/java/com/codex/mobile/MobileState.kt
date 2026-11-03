package com.codex.mobile

import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.net.Uri
import androidx.compose.runtime.*
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID

data class ChatLine(val id: String, val role: String, val text: String)
data class SessionLine(val id: String, val title: String)
data class FeatureGroup(val title: String, val key: String, val methods: List<String>)

class MobileState(private val context: Context) {
    val engine = CodexRpcEngine(context)
    private val mainHandler = Handler(Looper.getMainLooper())
    val chat = mutableStateListOf<ChatLine>()
    val sessions = mutableStateListOf<SessionLine>()
    val models = mutableStateListOf<String>()
    val activities = mutableStateListOf<ChatLine>()
    val featureGroups = mutableStateListOf<FeatureGroup>()
    var connected by mutableStateOf(false)
    var busy by mutableStateOf(false)
    var status by mutableStateOf("连接中")
    var threadId by mutableStateOf<String?>(null)
    var turnId by mutableStateOf<String?>(null)
    var selectedModel by mutableStateOf("")
    var apiKey by mutableStateOf("")
    var pendingApproval by mutableStateOf<JSONObject?>(null)
    private val approvalQueue = ArrayDeque<JSONObject>()
    var loginCode by mutableStateOf("")
    var loginUrl by mutableStateOf("")
    var lastError by mutableStateOf<String?>(null)
    var writeAccess by mutableStateOf(false)
    var darkMode by mutableStateOf(false)
    var selectedGroup by mutableStateOf("thread")
    var selectedMethod by mutableStateOf("thread/list")
    var responseJson by mutableStateOf("")
    val attachments = mutableStateListOf<File>()
    internal val fileManager by lazy { WorkspaceFiles(engine.workspace()) }
    var workspaceRevision by mutableIntStateOf(0)
    var sessionCursor by mutableStateOf<String?>(null)
    var sessionLoading by mutableStateOf(false)
    var sessionHasMore by mutableStateOf(false)
    var accountDescription by mutableStateOf("尚未检查登录状态")
    var operationRunning by mutableStateOf(false)
    var lastOperationMethod by mutableStateOf("")
    fun workspaceChanged() { workspaceRevision++ }
    fun editWorkspace(action: () -> Unit) {
        try { action(); workspaceChanged() }
        catch (error: Exception) { lastError = error.message ?: error.javaClass.simpleName }
    }
    fun chooseMethod(group: String, method: String) {
        selectedGroup = group
        selectedMethod = method
        responseJson = ""
    }
    fun invokeQuickMethod(method: String, params: JSONObject = JSONObject()) {
        invokeMethod(method, params.toString())
    }

    private fun ui(block: () -> Unit) {
        mainHandler.post(block)
    }

    init {
        try {
            val raw = context.assets.open("rpc_methods.json").bufferedReader().use { it.readText() }
            val groups = JSONObject(raw).getJSONObject("groups")
            val iterator = groups.keys()
            while (iterator.hasNext()) {
                val key = iterator.next()
                val values = groups.getJSONArray(key)
                featureGroups.add(FeatureGroup(groupTitle(key), key,
                    (0 until values.length()).map { values.getString(it) }))
            }
        } catch (error: Exception) {
            lastError = "协议目录加载失败：" + error.message
        }
    }

    private fun groupTitle(key: String) = mapOf(
        "thread" to "会话与任务", "turn" to "执行与控制", "model" to "模型",
        "review" to "代码审查", "project" to "项目与仓库", "fs" to "文件管理",
        "mcpServer" to "MCP 服务器", "plugin" to "插件中心", "skills" to "技能",
        "config" to "配置", "account" to "账户与授权", "permissionProfile" to "权限管理",
        "command" to "终端", "process" to "进程", "app" to "应用与连接器",
        "remoteControl" to "远程控制", "experimentalFeature" to "实验功能",
        "feedback" to "反馈", "environment" to "运行环境", "memory" to "记忆",
        "hooks" to "事件钩子", "collaborationMode" to "协作模式"
    )[key] ?: key

    fun start() {
        if (connected) return
        status = "正在启动本地 Codex 核心"
        Thread({
            try {
                engine.start(object : CodexRpcEngine.Events {
                    override fun onReady() = ui {
                        connected = true
                        status = "本地核心已连接"
                        refresh()
                        getModels()
                        readAccount()
                    }
                    override fun onNotification(message: JSONObject) = ui { handleNotification(message) }
                    override fun onServerRequest(message: JSONObject) = ui {
                        val method = message.optString("method")
                        val id = message.opt("id")
                        if (method == "currentTime/read" && id != null) {
                            try {
                                engine.respond(id, JSONObject().put("currentTimeAt",
                                    System.currentTimeMillis() / 1000L))
                                addActivity("currentTime/read", "已返回本机时间")
                            } catch (error: Exception) {
                                lastError = "读取本机时间失败：" + error.message
                            }
                        } else {
                            if (pendingApproval == null) pendingApproval = message
                            else approvalQueue.addLast(message)
                            status = "等待人工确认"
                        }
                    }
                    override fun onDisconnected(reason: String) = ui {
                        connected = false
                        busy = false
                        context.stopService(Intent(context, CodexTaskService::class.java))
                        status = reason
                        if (pendingApproval != null || approvalQueue.isNotEmpty()) {
                            lastError = "连接中断，待审核请求尚未完成"
                            pendingApproval = null
                            approvalQueue.clear()
                        }
                    }
                })
            } catch (error: Exception) {
                ui {
                    connected = false
                    status = "本地核心启动失败"
                    lastError = error.message ?: error.javaClass.simpleName
                }
            }
        }, "codex-bootstrap").start()
    }

    private fun ask(method: String, params: JSONObject, done: (JSONObject) -> Unit = {}) {
        if (!connected) {
            lastError = "核心尚未连接：" + status
            return
        }
        engine.request(method, params).whenComplete { result, error ->
            ui {
                if (error != null) {
                    lastError = method + "：" + (error.cause?.message ?: error.message)
                    if (method == "thread/start" || method == "turn/start") {
                        busy = false
                        context.stopService(Intent(context, CodexTaskService::class.java))
                    }
                } else done(result)
            }
        }
    }

    fun refresh() = loadSessions(null)

    fun loadMoreSessions() {
        if (sessionHasMore && !sessionLoading) loadSessions(sessionCursor)
    }

    private fun loadSessions(cursor: String?) {
        if (!connected || sessionLoading) return
        sessionLoading = true
        val parameters = JSONObject().put("limit", 100)
        if (!cursor.isNullOrBlank()) parameters.put("cursor", cursor)
        engine.request("thread/list", parameters).whenComplete { result, error ->
            ui {
                sessionLoading = false
                if (error != null) {
                    lastError = "会话记录：" + (error.cause?.message ?: error.message)
                    return@ui
                }
                if (cursor == null) sessions.clear()
                val data = result.optJSONArray("data") ?: JSONArray()
                val existing = sessions.mapTo(HashSet()) { it.id }
                for (i in 0 until data.length()) {
                    val item = data.optJSONObject(i) ?: continue
                    val id = item.optString("id")
                    if (id.isBlank() || !existing.add(id)) continue
                    sessions.add(SessionLine(id, item.optString("name").takeIf { it.isNotBlank() }
                        ?: item.optString("preview").takeIf { it.isNotBlank() }
                        ?: "会话 " + id.take(8)))
                }
                sessionCursor = result.optString("nextCursor").takeIf { it.isNotBlank() }
                sessionHasMore = sessionCursor != null
            }
        }
    }

    fun readAccount() = ask("account/read", JSONObject()) { result ->
        val account = result.optJSONObject("account")
        accountDescription = when {
            account == null || account.optString("type").isBlank() -> "尚未登录 ChatGPT，可使用 API 密钥"
            account.optString("email").isNotBlank() -> account.optString("email")
            else -> "已登录：" + account.optString("type")
        }
    }

    fun getModels() = ask("model/list", JSONObject().put("limit", 100)) { response ->
        models.clear()
        val data = response.optJSONArray("data") ?: JSONArray()
        for (i in 0 until data.length()) {
            val item = data.optJSONObject(i) ?: continue
            val id = item.optString("id").ifBlank { item.optString("model") }
            if (id.isNotBlank() && !models.contains(id)) models.add(id)
        }
    }

    fun newChat() {
        if (busy) { lastError = "任务执行中，请先停止后再新建会话"; return }
        threadId = null
        turnId = null
        busy = false
        chat.clear()
        activities.clear()
        status = if (connected) "新会话准备就绪" else "等待核心连接"
    }

    fun openThread(id: String) {
        if (busy) { lastError = "已有任务执行中，请先中断或等待"; return }
        ask("thread/resume", JSONObject().put("threadId", id)) {
            threadId = id
            chat.clear()
            status = "正在恢复会话"
            activities.clear()
            ask("thread/read", JSONObject().put("threadId", id).put("includeTurns", true)) { response ->
                val turns = response.optJSONObject("thread")?.optJSONArray("turns") ?: JSONArray()
                for (i in 0 until turns.length()) {
                    val items = turns.optJSONObject(i)?.optJSONArray("items") ?: continue
                    for (j in 0 until items.length()) {
                        val item = items.optJSONObject(j) ?: continue
                        val kind = item.optString("type")
                        val text = when (kind) {
                            "agentMessage" -> item.optString("text")
                            "userMessage" -> {
                                val contents = item.optJSONArray("content") ?: JSONArray()
                                (0 until contents.length()).mapNotNull { index ->
                                    val part = contents.optJSONObject(index) ?: return@mapNotNull null
                                    when (part.optString("type")) {
                                        "text" -> part.optString("text")
                                        "localImage", "image" -> "[图片]"
                                        "localAudio", "audio" -> "[音频]"
                                        "mention" -> "[文件：" + part.optString("name") + "]"
                                        else -> null
                                    }
                                }.joinToString("\n")
                            }
                            else -> ""
                        }
                        if (text.isNotBlank()) chat.add(ChatLine(UUID.randomUUID().toString(),
                            if (kind == "agentMessage") "assistant" else "user", text))
                    }
                }
                status = "会话已恢复"
            }
        }
    }

    fun send(prompt: String) {
        if (prompt.isBlank() || busy) return
        if (!connected) { lastError = "Codex 原生核心尚未连接"; return }
        try {
            context.startForegroundService(Intent(context, CodexTaskService::class.java))
        } catch (error: Exception) {
            lastError = "无法启动后台任务服务：" + error.message
            return
        }
        busy = true
        chat.add(ChatLine(UUID.randomUUID().toString(), "user", prompt))
        val chosenFiles = attachments.toList()
        attachments.clear()
        val id = threadId
        if (id == null) {
            try {
                val args = JSONObject().put("cwd", engine.workspace().absolutePath)
                    .put("sandbox", if (writeAccess) "workspace-write" else "read-only")
                if (selectedModel.isNotBlank()) args.put("model", selectedModel)
                // Always preserve interactive approvals.
                ask("thread/start", args) { result ->
                    val nextId = result.optJSONObject("thread")?.optString("id").orEmpty()
                    if (nextId.isBlank()) {
                        busy = false
                        context.stopService(Intent(context, CodexTaskService::class.java))
                        lastError = "Codex 未返回会话 ID"
                    } else {
                        threadId = nextId
                        refresh()
                        startTurn(nextId, prompt, chosenFiles)
                    }
                }
            } catch (error: Exception) {
                busy = false
                context.stopService(Intent(context, CodexTaskService::class.java))
                lastError = error.message
            }
        } else startTurn(id, prompt, chosenFiles)
    }

    private fun startTurn(id: String, prompt: String, files: List<File>) {
        val input = JSONArray().put(JSONObject().put("type", "text").put("text", prompt))
        for (file in files) {
            val extension = file.extension.lowercase()
            val kind = when {
                extension in setOf("jpg", "jpeg", "png", "webp") -> "localImage"
                extension in setOf("mp3", "wav", "m4a", "ogg") -> "localAudio"
                else -> "mention"
            }
            val part = JSONObject().put("type", kind).put("path", file.absolutePath)
            if (kind == "mention") part.put("name", file.name)
            input.put(part)
        }
        val parameters = JSONObject().put("threadId", id).put("input", input)
        if (selectedModel.isNotBlank()) parameters.put("model", selectedModel)
        ask("turn/start", parameters) { result ->
            turnId = result.optJSONObject("turn")?.optString("id")
            status = "Codex 正在处理"
        }
    }

    fun interrupt() {
        val id = threadId ?: return
        val turn = turnId ?: run { lastError = "暂无运行中的任务"; return }
        ask("turn/interrupt", JSONObject().put("threadId", id).put("turnId", turn)) {
            status = "已请求停止"
        }
    }

    fun archive(id: String) = ask("thread/archive", JSONObject().put("threadId", id)) {
        if (id == threadId) newChat()
        refresh()
    }

    fun rename(id: String, name: String) = ask("thread/name/set",
        JSONObject().put("threadId", id).put("name", name)) { refresh() }

    fun invokeMethod(method: String, args: String) {
        if (!connected) { lastError = "Codex 原生核心尚未连接"; return }
        if (operationRunning) { lastError = "上一个功能调用尚未结束"; return }
        val payload = try { JSONObject(args.ifBlank { "{}" }) }
        catch (error: Exception) { lastError = "接口参数错误：" + error.message; return }
        operationRunning = true
        lastOperationMethod = method
        responseJson = "正在调用 " + method + "…"
        engine.request(method, payload).whenComplete { response, error ->
            ui {
                operationRunning = false
                if (error != null) {
                    responseJson = "调用失败：" + (error.cause?.message ?: error.message)
                    lastError = method + "：" + (error.cause?.message ?: error.message)
                } else {
                    responseJson = response.toString(2)
                    addActivity(method, responseJson)
                    if (method.startsWith("fs/") || method.startsWith("command/")) workspaceChanged()
                    if (method.startsWith("thread/")) refresh()
                }
            }
        }
    }

    fun resolveApproval(accept: Boolean, args: String = "") {
        val request = pendingApproval ?: return
        val method = request.optString("method")
        val id = request.opt("id") ?: return
        try {
            if (!accept) {
                if (method in setOf("item/commandExecution/requestApproval", "item/fileChange/requestApproval")) {
                    engine.respond(id, JSONObject().put("decision", "decline"))
                } else if (method == "mcpServer/elicitation/request") {
                    engine.respond(id, JSONObject().put("action", "decline"))
                } else if (method == "item/permissions/requestApproval") {
                    // No requested capability is granted.
                    engine.respond(id, JSONObject().put("permissions", JSONObject())
                        .put("scope", "turn"))
                } else engine.reject(id, "用户明确拒绝了该操作")
            } else {
                val result = if (args.isNotBlank()) JSONObject(args) else when (method) {
                    "item/commandExecution/requestApproval", "item/fileChange/requestApproval" ->
                        JSONObject().put("decision", "accept")
                    else -> throw IllegalArgumentException("此操作需要填写完整 JSON 审批回应")
                }
                engine.respond(id, result)
            }
            pendingApproval = if (approvalQueue.isEmpty()) null else approvalQueue.removeFirst()
            status = if (pendingApproval == null) "授权已响应" else "还有待处理的授权请求"
        } catch (error: Exception) {
            lastError = "响应失败：" + error.message
        }
    }

    fun importAttachment(uri: Uri) {
        try {
            val resolver = context.contentResolver
            val mime = resolver.getType(uri) ?: ""
            val originalName = resolver.query(uri,
                arrayOf(android.provider.OpenableColumns.DISPLAY_NAME),
                null, null, null)?.use { cursor ->
                    val column = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                    if (column >= 0 && cursor.moveToFirst()) cursor.getString(column) else null
                }.orEmpty()
            val clean = originalName.replace(Regex("[^a-zA-Z0-9_.\\-]"), "_")
                .take(120).ifBlank { "upload" }
            val ext = clean.substringAfterLast('.', "").lowercase()
            val fallbackExtension = when {
                mime == "image/jpeg" -> "jpg"
                mime == "image/png" -> "png"
                mime == "image/webp" -> "webp"
                mime.startsWith("audio/") -> "m4a"
                else -> "bin"
            }
            val suffix = if (ext.isNotBlank()) ext else fallbackExtension
            val name = clean.substringBeforeLast('.', clean).take(70)
            val target = File(engine.workspace(),
                name + "-" + UUID.randomUUID() + "." + suffix)
            resolver.openInputStream(uri).use { stream ->
                requireNotNull(stream) { "无法读取文件内容" }
                target.outputStream().use { stream.copyTo(it) }
            }
            attachments.add(target)
            workspaceChanged()
        } catch (error: Exception) {
            lastError = "附件导入失败：" + error.message
        }
    }

    fun loginChatGpt() {
        ask("account/login/start", JSONObject().put("type", "chatgptDeviceCode")) { result ->
            loginCode = result.optString("userCode")
            loginUrl = result.optString("verificationUrl")
            if (!loginUrl.startsWith("https://") || loginCode.isBlank()) {
                lastError = "ChatGPT 设备授权参数缺失"
                return@ask
            }
            val intent = android.content.Intent(android.content.Intent.ACTION_VIEW,
                Uri.parse(loginUrl))
            try { context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
            catch (error: Exception) { lastError = "打开授权页失败：" + error.message }
        }
    }

    fun logout() = ask("account/logout", JSONObject()) {
        loginCode = ""
        loginUrl = ""
        accountDescription = "已退出 ChatGPT"
        status = "账号已退出"
    }

    fun saveKey(value: String) {
        try {
            SecretStore(context).save(value.trim())
            apiKey = ""
            connected = false
            Thread({
                engine.stop()
                ui { start() }
            }, "codex-auth-restart").start()
        } catch (error: Exception) {
            lastError = "密钥保存失败：" + error.message
        }
    }

    private fun handleNotification(message: JSONObject) {
        val method = message.optString("method")
        val params = message.optJSONObject("params") ?: JSONObject()
        when (method) {
            "item/agentMessage/delta" -> {
                val delta = params.optString("delta")
                val key = params.optString("itemId").takeIf { it.isNotBlank() }
                    ?.let { "agent:" + it }
                if (delta.isNotBlank()) {
                    val index = if (key != null) chat.indexOfLast { it.id == key } else -1
                    if (index >= 0) {
                        chat[index] = chat[index].copy(text = chat[index].text + delta)
                    } else {
                        chat.add(ChatLine(key ?: UUID.randomUUID().toString(), "assistant", delta))
                    }
                }
            }
            "item/completed" -> {
                val item = params.optJSONObject("item") ?: JSONObject()
                val txt = item.optString("text")
                if (item.optString("type") == "agentMessage" && txt.isNotBlank()) {
                    val key = item.optString("id").takeIf { it.isNotBlank() }
                        ?.let { "agent:" + it }
                    val index = if (key != null) chat.indexOfLast { it.id == key } else -1
                    if (index >= 0) chat[index] = chat[index].copy(text = txt)
                    else chat.add(ChatLine(key ?: UUID.randomUUID().toString(), "assistant", txt))
                }
                if (item.optString("type") == "fileChange") workspaceChanged()
                addActivity(method, item.optString("type"))
            }
            "turn/started" -> {
                busy = true
                turnId = params.optJSONObject("turn")?.optString("id")
                status = "正在执行"
            }
            "turn/completed" -> {
                context.stopService(Intent(context, CodexTaskService::class.java))
                busy = false
                turnId = null
                val turn = params.optJSONObject("turn")
                status = when (turn?.optString("status")) {
                    "failed" -> "执行失败"
                    "interrupted" -> "已停止"
                    else -> "已完成"
                }
                if (status == "执行失败") {
                    lastError = turn?.optJSONObject("error")?.optString("message")
                        ?.takeIf { it.isNotBlank() } ?: "任务执行失败，可在执行过程查看详情"
                }
                workspaceChanged()
                refresh()
            }
            "thread/name/updated", "thread/archived", "thread/started" -> refresh()
            "account/login/completed" -> {
                loginCode = ""
                loginUrl = ""
                status = "ChatGPT 授权流程已结束"
                readAccount()
                getModels()
            }
            "error" -> { lastError = params.optString("message").ifBlank { params.toString() } }
            "turn/plan/updated", "turn/diff/updated", "item/commandExecution/outputDelta",
            "item/fileChange/patchUpdated", "item/mcpToolCall/progress",
            "item/started" -> addActivity(method, params.toString())
            else -> if (method.isNotBlank()) addActivity(method, "")
        }
    }

    private fun addActivity(name: String, data: String) {
        activities.add(ChatLine(UUID.randomUUID().toString(), "event",
            if (data.isBlank()) name else name + " · " + data.take(16000)))
        if (activities.size > 400) activities.removeAt(0)
    }

    fun restartCore() {
        status = "正在重新连接本地核心"
        connected = false
        Thread({
            engine.stop()
            ui { start() }
        }, "codex-runtime-restart").start()
    }

    fun close() = engine.close()
}
