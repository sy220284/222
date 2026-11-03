package com.codex.mobile

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import org.json.JSONArray
import org.json.JSONObject

@Composable
internal fun ApprovalDialog(
    request: JSONObject,
    onDecline: () -> Unit,
    onAccept: (String) -> Unit
) {
    val context = LocalContext.current
    val method = request.optString("method")
    val params = request.optJSONObject("params") ?: JSONObject()
    val questions = params.optJSONArray("questions") ?: JSONArray()
    val isQuestion = method == "item/tool/requestUserInput"
    val isMcp = method == "mcpServer/elicitation/request"
    val isPermission = method == "item/permissions/requestApproval"
    val isSimple = method == "item/commandExecution/requestApproval" ||
        method == "item/fileChange/requestApproval" ||
        method == "applyPatchApproval" || method == "execCommandApproval"
    val responses = remember(request) { mutableStateMapOf<String, String>() }
    var raw by remember(request) { mutableStateOf("") }
    var advanced by remember(request) { mutableStateOf(false) }
    val schema = params.optJSONObject("requestedSchema")
    val props = schema?.optJSONObject("properties")
    val required = schema?.optJSONArray("required") ?: JSONArray()
    val requiredNames = (0 until required.length()).map { required.optString(it) }.toSet()

    fun buildResponse(): String {
        if (advanced) {
            require(raw.isNotBlank()) { "请填写完整的 JSON 回应" }
            return JSONObject(raw).toString()
        }
        if (isSimple) return ""
        if (isPermission) {
            val requested = params.optJSONObject("permissions") ?: JSONObject()
            return JSONObject().put("permissions", requested).put("scope", "turn").toString()
        }
        if (isQuestion) {
            val answers = JSONObject()
            for (index in 0 until questions.length()) {
                val item = questions.getJSONObject(index)
                val id = item.getString("id")
                val answer = responses[id]?.trim().orEmpty()
                require(answer.isNotBlank()) { "请回答：" + item.optString("question") }
                answers.put(id, JSONObject().put("answers", JSONArray().put(answer)))
            }
            return JSONObject().put("answers", answers).toString()
        }
        if (isMcp) {
            if (params.optString("mode") == "url") {
                return JSONObject().put("action", "accept").toString()
            }
            val content = JSONObject()
            val keys = props?.keys()
            while (keys?.hasNext() == true) {
                val key = keys.next()
                val property = props.optJSONObject(key) ?: continue
                val value = responses[key]?.trim().orEmpty()
                require(key !in requiredNames || value.isNotEmpty()) { "请填写：" + key }
                if (value.isEmpty()) continue
                val kind = property.optString("type")
                content.put(key, when (kind) {
                    "boolean" -> when (value.lowercase()) {
                        "true" -> true
                        "false" -> false
                        else -> throw IllegalArgumentException(key + " 需要是 true 或 false")
                    }
                    "integer" -> value.toLongOrNull()
                        ?: throw IllegalArgumentException(key + " 需要整数")
                    "number" -> value.toDoubleOrNull()
                        ?: throw IllegalArgumentException(key + " 需要数字")
                    "object" -> JSONObject(value)
                    "array" -> JSONArray(value)
                    else -> value
                })
            }
            return JSONObject().put("action", "accept").put("content", content).toString()
        }
        throw IllegalArgumentException("该类型需要完整 JSON 回应")
    }

    Dialog(onDismissRequest = {}) {
        Surface(shape = RoundedCornerShape(26.dp),
            color = MaterialTheme.colorScheme.surface, tonalElevation = 5.dp,
            modifier = Modifier.fillMaxWidth().heightIn(max = 630.dp)) {
            Column(Modifier.padding(20.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Outlined.VerifiedUser, null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(27.dp))
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(if (isQuestion) "Codex 需要你的回答" else if (isMcp) "外部工具请求确认"
                            else "需要人工确认", style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold)
                        Text(method, style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.height(13.dp))
                Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState())) {
                    if (isQuestion && !advanced) {
                        for (index in 0 until questions.length()) {
                            val question = questions.getJSONObject(index)
                            val id = question.optString("id")
                            Text(question.optString("question"), fontWeight = FontWeight.Medium)
                            Spacer(Modifier.height(7.dp))
                            val options = question.optJSONArray("options")
                            if (options != null && options.length() > 0) {
                                for (choiceIndex in 0 until options.length()) {
                                    val option = options.optJSONObject(choiceIndex) ?: continue
                                    val label = option.optString("label")
                                    FilterChip(selected = responses[id] == label,
                                        onClick = { responses[id] = label },
                                        label = { Text(label) },
                                        leadingIcon = {
                                            if (responses[id] == label)
                                                Icon(Icons.Outlined.Check, null, Modifier.size(15.dp))
                                        })
                                }
                            }
                            if (options == null || question.optBoolean("isOther")) {
                                OutlinedTextField(value = responses[id].orEmpty(),
                                    onValueChange = { responses[id] = it },
                                    label = { Text("你的回答") },
                                    modifier = Modifier.fillMaxWidth(),
                                    visualTransformation = if (question.optBoolean("isSecret"))
                                        androidx.compose.ui.text.input.PasswordVisualTransformation()
                                    else androidx.compose.ui.text.input.VisualTransformation.None)
                            }
                            Spacer(Modifier.height(15.dp))
                        }
                    } else if (isMcp && !advanced) {
                        Text(params.optString("message"), style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(12.dp))
                        if (params.optString("mode") == "url") {
                            val url = params.optString("url")
                            OutlinedButton(onClick = {
                                if (url.startsWith("https://")) {
                                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                                }
                            }, enabled = url.startsWith("https://")) {
                                Icon(Icons.Outlined.OpenInBrowser, null, Modifier.size(17.dp))
                                Spacer(Modifier.width(7.dp))
                                Text("打开验证页面")
                            }
                        } else if (props != null) {
                            val keys = props.keys()
                            while (keys.hasNext()) {
                                val name = keys.next()
                                val property = props.optJSONObject(name) ?: continue
                                OutlinedTextField(value = responses[name].orEmpty(),
                                    onValueChange = { responses[name] = it },
                                    label = { Text(name + if (name in requiredNames) " *" else "") },
                                    modifier = Modifier.fillMaxWidth(),
                                    supportingText = {
                                        if (property.optString("description").isNotBlank())
                                            Text(property.optString("description").take(190))
                                    })
                                Spacer(Modifier.height(7.dp))
                            }
                        } else {
                            Text("表单结构需要高级 JSON 回应。")
                        }
                    } else if (isPermission && !advanced) {
                        Text("Codex 请求额外访问权限，请确认范围。",
                            style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(8.dp))
                        SelectionContainer {
                            Text(params.optJSONObject("permissions")?.toString(2) ?: "{}",
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace))
                        }
                        Spacer(Modifier.height(6.dp))
                        Text("仅授权当前任务；Android 原生隔离仍需要设备级验收。",
                            color = MaterialTheme.colorScheme.error,
                            style = MaterialTheme.typography.labelSmall)
                    } else if (isSimple && !advanced) {
                        SelectionContainer {
                            Text(params.toString(2).take(4000),
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = FontFamily.Monospace))
                        }
                    } else if (!advanced) {
                        Text("此请求类型需要手动填写符合协议的 JSON 结果。")
                    }
                    if (advanced) {
                        OutlinedTextField(value = raw, onValueChange = { raw = it },
                            modifier = Modifier.fillMaxWidth(), minLines = 3, maxLines = 10,
                            label = { Text("完整 JSON 回应") },
                            textStyle = MaterialTheme.typography.bodySmall.copy(
                                fontFamily = FontFamily.Monospace))
                    }
                    TextButton(onClick = { advanced = !advanced }) {
                        Icon(Icons.Outlined.Tune, null, Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(if (advanced) "返回常规表单" else "高级 JSON 回应")
                    }
                }
                Spacer(Modifier.height(10.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    OutlinedButton(onClick = onDecline) { Text("拒绝") }
                    Spacer(Modifier.width(9.dp))
                    Button(onClick = {
                        try { onAccept(buildResponse()) }
                        catch (error: Exception) {
                            // Keep the dialog open and show the precise field error.
                            responses["_validationError"] = error.message ?: "回应格式错误"
                        }
                    }) { Text(if (isQuestion) "提交回答" else "确认执行") }
                }
                responses["_validationError"]?.let {
                    Spacer(Modifier.height(8.dp))
                    Text(it, color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.labelSmall)
                }
            }
        }
    }
}
