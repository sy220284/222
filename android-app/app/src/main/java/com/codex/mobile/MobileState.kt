package com.codex.mobile

import android.app.Activity
import android.net.Uri
import androidx.compose.runtime.*
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID

data class ChatLine(val id: String, val role: String, val text: String)
data class SessionLine(val id: String, val title: String)
data class FeatureGroup(val title: String, val key: String, val methods: List<String>)

class MobileState(private val activity: Activity) {
    val engine = CodexRpcEngine(activity)
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
    var lastError by mutableStateOf<String?>(null)
    var writeAccess by mutableStateOf(false)
    var darkMode by mutableStateOf(false)
    var selectedGroup by mutableStateOf("thread")
    var selectedMethod by mutableStateOf("thread/list")
    var responseJson by mutableStateOf("")
    var attachment by mutableStateOf<File?>(null)

    private fun ui(block: () -> Unit) = activity.runOnUiThread(block)

    init {
        try {
            val raw = activity.assets.open("rpc_methods.json").bufferedReader().use { it.readText() }
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
                    }
                    override fun onNotification(message: JSONObject) = ui { handleNotification(message) }
                    override fun onServerRequest(message: JSONObject) = ui {
                        pendingApproval = message
                        status = "等待人工确认"
                    }
                    override fun onDisconnected(reason: String) = ui {
                        connected = false
                        busy = false
                        status = reason
                        if (pendingApproval != null) {
                            lastError = "连接中断，待审核请求尚未完成"
                            pendingApproval = null
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
                    busy = false
                } else done(result)
            }
        }
    }

    fun refresh() = ask("thread/list", JSONObject().put("limit", 100)) { result ->
        sessions.clear()
        val data = result.optJSONArray("data") ?: JSONArray()
        for (i in 0 until data.length()) {
            val item = data.optJSONObject(i) ?: continue
            val id = item.optString("id")
            if (id.isBlank()) continue
            sessions.add(SessionLine(id, item.optString("name").takeIf { it.isNotBlank() }
                ?: item.optString("preview").takeIf { it.isNotBlank() }
                ?: "会话 " + id.take(8)))
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
                            "userMessage" -> item.optJSONArray("content")?.toString() ?: ""
                            else -> ""
                        }
                        if (text.isNotBlank()) chat.add(ChatLine(UUID.randomUUID().toString(),
                            if (kind == "agentMessage") "assistant" else "user", text))
                    }
                }
            }
        }
    }

    fun send(prompt: String) {
        if (prompt.isBlank() || busy) return
        busy = true
        chat.add(ChatLine(UUID.randomUUID().toString(), "user", prompt))
        val file = attachment
        attachment = null
        val id = threadId
        if (id == null) {
            try {
                val args = JSONObject().put("cwd", engine.workspace().absolutePath)
                if (selectedModel.isNotBlank()) args.put("model", selectedModel)
                // Always preserve interactive approvals.
                ask("thread/start", args) { result ->
                    val nextId = result.optJSONObject("thread")?.optString("id").orEmpty()
                    if (nextId.isBlank()) {
                        busy = false
                        lastError = "Codex 未返回会话 ID"
                    } else {
                        threadId = nextId
                        refresh()
                        startTurn(nextId, prompt, file)
                    }
                }
            } catch (error: Exception) {
                busy = false
                lastError = error.message
            }
        } else startTurn(id, prompt, file)
    }

    private fun startTurn(id: String, prompt: String, file: File?) {
        val input = JSONArray().put(JSONObject().put("type", "text").put("text", prompt))
        if (file != null) {
            val ext = file.extension.lowercase()
            val type = when {
                ext in setOf("jpg", "jpeg", "png", "webp") -> "localImage"
                ext in setOf("mp3", "wav", "m4a", "ogg") -> "localAudio"
                else -> "mention"
            }
            val part = JSONObject().put("type", type).put("path", file.absolutePath)
            if (type == "mention") part.put("name", file.name)
            input.put(part)
        }
        ask("turn/start", JSONObject().put("threadId", id).put("input", input)) { result ->
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
        try {
            responseJson = "执行中：" + method
            ask(method, JSONObject(args.ifBlank { "{}" })) { responseJson = it.toString(2) }
        } catch (error: Exception) {
            lastError = "接口参数错误：" + error.message
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
                } else engine.reject(id, "用户明确拒绝了该操作")
            } else {
                val result = if (args.isNotBlank()) JSONObject(args) else when (method) {
                    "item/commandExecution/requestApproval", "item/fileChange/requestApproval" ->
                        JSONObject().put("decision", "accept")
                    else -> throw IllegalArgumentException("此操作需要填写完整 JSON 审批回应")
                }
                engine.respond(id, result)
            }
            pendingApproval = null
            status = "授权已响应"
        } catch (error: Exception) {
            lastError = "响应失败：" + error.message
        }
    }

    fun importAttachment(uri: Uri) {
        try {
            val mime = activity.contentResolver.getType(uri) ?: ""
            val extension = when {
                mime.startsWith("image/") -> "png"
                mime.startsWith("audio/") -> "m4a"
                else -> "txt"
            }
            val file = File(engine.workspace(), "import-" + UUID.randomUUID() + "." + extension)
            activity.contentResolver.openInputStream(uri).use { stream ->
                requireNotNull(stream) { "无法读取文件内容" }
                file.outputStream().use { stream.copyTo(it) }
            }
            attachment = file
        } catch (error: Exception) {
            lastError = "附件导入失败：" + error.message
        }
    }

    fun saveKey(value: String) {
        try {
            SecretStore(activity).save(value.trim())
            apiKey = ""
            engine.stop()
            connected = false
            start()
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
                if (delta.isNotBlank()) {
                    val last = chat.lastOrNull()
                    if (last?.role == "assistant") chat[chat.lastIndex] = last.copy(text = last.text + delta)
                    else chat.add(ChatLine(UUID.randomUUID().toString(), "assistant", delta))
                }
            }
            "item/completed" -> {
                val item = params.optJSONObject("item") ?: JSONObject()
                val txt = item.optString("text")
                if (item.optString("type") == "agentMessage" && txt.isNotBlank()) {
                    val last = chat.lastOrNull()
                    if (last?.role == "assistant") chat[chat.lastIndex] = last.copy(text = txt)
                    else chat.add(ChatLine(UUID.randomUUID().toString(), "assistant", txt))
                }
                addActivity(method, item.optString("type"))
            }
            "turn/started" -> {
                busy = true
                turnId = params.optJSONObject("turn")?.optString("id")
                status = "正在执行"
            }
            "turn/completed" -> {
                busy = false
                turnId = null
                status = "已完成"
                refresh()
            }
            "thread/name/updated", "thread/archived", "thread/started" -> refresh()
            "error" -> { lastError = params.optString("message").ifBlank { params.toString() } }
            "turn/plan/updated", "turn/diff/updated", "item/commandExecution/outputDelta",
            "item/fileChange/patchUpdated", "item/mcpToolCall/progress",
            "item/started" -> addActivity(method, params.toString())
            else -> if (method.isNotBlank()) addActivity(method, "")
        }
    }

    private fun addActivity(name: String, data: String) {
        activities.add(ChatLine(UUID.randomUUID().toString(), "event",
            if (data.isBlank()) name else name + " · " + data.take(200)))
        if (activities.size > 400) activities.removeAt(0)
    }

    fun close() = engine.close()
}
