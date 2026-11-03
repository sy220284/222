package com.codex.mobile

import android.content.Context
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ExpandMore
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

/**
 * Builds a typed form from the exact JSON Schema bundled by upstream Codex.
 * A user can still switch to raw JSON for nested and experimental capabilities.
 */
internal data class ParameterField(
    val key: String,
    val label: String,
    val description: String,
    val kind: String,
    val required: Boolean,
    val choices: List<String>
)

internal data class RpcVariant(val title: String, val discriminator: String)

internal object RpcForms {
    private fun findSource(context: Context, method: String): JSONObject? {
        val definitions = JSONObject(context.assets.open("rpc_schemas.json")
            .bufferedReader().use { it.readText() }).getJSONArray("methods")
        for (i in 0 until definitions.length()) {
            val entry = definitions.getJSONObject(i)
            if (entry.optString("method") == method) {
                val type = entry.optString("paramsType")
                val namespace = entry.optString("namespace")
                if (type.isBlank() || namespace.isBlank() || type == "null") return null
                return try {
                    JSONObject(context.assets.open(namespace + "/" + type + ".json")
                        .bufferedReader().use { it.readText() })
                } catch (missing: java.io.FileNotFoundException) {
                    return null
                }
            }
        }
        return null
    }

    fun variants(context: Context, method: String): List<RpcVariant> {
        val root = findSource(context, method) ?: return emptyList()
        val variants = root.optJSONArray("oneOf") ?: return emptyList()
        return (0 until variants.length()).mapNotNull { index ->
            val item = variants.optJSONObject(index) ?: return@mapNotNull null
            val label = item.optString("title").ifBlank { "方案 " + (index + 1) }
            val discriminator = item.optJSONObject("properties")
                ?.optJSONObject("type")?.optJSONArray("enum")?.optString(0).orEmpty()
            RpcVariant(label, discriminator)
        }
    }

    fun fields(context: Context, method: String, variantIndex: Int = 0): List<ParameterField> {
        val root = findSource(context, method) ?: return emptyList()
        val selected = root.optJSONArray("oneOf")?.optJSONObject(variantIndex)
        val required = selected?.optJSONArray("required") ?: root.optJSONArray("required") ?: JSONArray()
        val requiredNames = (0 until required.length()).map { required.optString(it) }.toSet()
        val properties = selected?.optJSONObject("properties")
            ?: root.optJSONObject("properties") ?: return emptyList()
        val definitions = root.optJSONObject("definitions") ?: JSONObject()
        val result = mutableListOf<ParameterField>()
        val keys = properties.keys()
        while (keys.hasNext()) {
            val key = keys.next()
            val property = properties.optJSONObject(key) ?: continue
            val nested = property.optJSONArray("anyOf")
                ?: property.optJSONArray("oneOf")
                ?: property.optJSONArray("allOf")
            val resolved = if (nested != null) {
                (0 until nested.length()).mapNotNull { nested.optJSONObject(it) }
                    .firstOrNull {
                        it.has("\$ref") || it.optString("type") != "null"
                    } ?: property
            } else property
            val reference = resolved.optString("\$ref")
            val schema = if (reference.startsWith("#/definitions/")) {
                definitions.optJSONObject(reference.substringAfterLast('/')) ?: resolved
            } else resolved
            val typeValue = schema.opt("type")
            val kind = if (schema.has("enum")) "enum" else when (typeValue) {
                is JSONArray -> (0 until typeValue.length())
                    .map { typeValue.optString(it) }.firstOrNull { it != "null" } ?: "string"
                is String -> typeValue
                else -> if (schema.has("properties")) "object" else "string"
            }
            val choices = schema.optJSONArray("enum")?.let { values ->
                (0 until values.length()).map { values.optString(it) }
            } ?: emptyList()
            result.add(ParameterField(
                key = key, label = readableName(key),
                description = property.optString("description").take(210),
                kind = kind, required = key in requiredNames, choices = choices
            ))
        }
        return result.sortedWith(compareByDescending<ParameterField> { it.required }
            .thenBy { it.key })
    }

    private fun readableName(key: String) = mapOf(
        "threadId" to "会话 ID", "turnId" to "任务 ID", "cwd" to "工作目录",
        "model" to "模型", "modelProvider" to "模型提供方", "limit" to "每页数量",
        "cursor" to "下一页位置", "name" to "名称", "input" to "输入内容",
        "sandbox" to "沙箱权限", "approvalPolicy" to "审批策略",
        "includeTurns" to "包含历史回合", "includeHidden" to "包含隐藏模型",
        "query" to "搜索内容", "path" to "文件路径", "config" to "配置",
        "includeDisabled" to "包含已禁用项", "prompt" to "任务要求"
    )[key] ?: key

    fun build(fields: List<ParameterField>, values: Map<String, String>): JSONObject {
        val result = JSONObject()
        for (field in fields) {
            val input = values[field.key]?.trim().orEmpty()
            if (field.required && input.isBlank()) {
                throw IllegalArgumentException("请填写必填字段：" + field.label)
            }
            if (input.isBlank()) continue
            when (field.kind) {
                "boolean" -> result.put(field.key, when (input.lowercase()) {
                    "true" -> true
                    "false" -> false
                    else -> throw IllegalArgumentException(field.label + " 必须是 true 或 false")
                })
                "integer" -> result.put(field.key,
                    input.toLongOrNull()
                        ?: throw IllegalArgumentException(field.label + " 必须是整数"))
                "number" -> result.put(field.key,
                    input.toDoubleOrNull()
                        ?: throw IllegalArgumentException(field.label + " 必须是数字"))
                "object" -> result.put(field.key, JSONObject(input))
                "array" -> result.put(field.key, JSONArray(input))
                else -> result.put(field.key, input)
            }
        }
        return result
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RpcSchemaForm(state: MobileState, method: String,
                           onChange: (String) -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val variants = remember(method) { RpcForms.variants(context, method) }
    var variantIndex by remember(method) { mutableIntStateOf(0) }
    val fields = remember(method, variantIndex) {
        try { RpcForms.fields(context, method, variantIndex) }
        catch (error: Exception) {
            state.lastError = "读取接口表单失败：" + error.message
            emptyList()
        }
    }
    val values = remember(method, variantIndex) {
        mutableStateMapOf<String, String>().apply {
            variants.getOrNull(variantIndex)?.discriminator?.takeIf { it.isNotBlank() }
                ?.let { put("type", it) }
            state.threadId?.let { put("threadId", it) }
            state.turnId?.let { put("turnId", it) }
            try { put("cwd", state.engine.workspace().absolutePath) }
            catch (error: Exception) { state.lastError = error.message }
        }
    }
    if (variants.isNotEmpty()) {
        var menu by remember(method) { mutableStateOf(false) }
        Column(modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp)) {
            Text("请求类型", style = MaterialTheme.typography.labelMedium)
            Spacer(Modifier.height(6.dp))
            Box {
                OutlinedButton(onClick = { menu = true },
                    modifier = Modifier.fillMaxWidth()) {
                    Text(variants.getOrNull(variantIndex)?.title ?: "选择类型",
                        modifier = Modifier.weight(1f))
                    Icon(Icons.Outlined.ExpandMore, null)
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    variants.forEachIndexed { index, option ->
                        DropdownMenuItem(text = { Text(option.title) }, onClick = {
                            variantIndex = index
                            menu = false
                        })
                    }
                }
            }
        }
    }
    if (fields.isEmpty()) {
        Text("此接口没有可自动生成的字段，可切换高级 JSON 参数。",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        return
    }
    LaunchedEffect(method, variantIndex) {
        onChange(serialize(fields, values))
    }
    fields.forEach { field ->
        val value = values[field.key].orEmpty()
        Column(modifier = Modifier.fillMaxWidth().padding(bottom = 10.dp)) {
            Text(
                field.label + if (field.required) " *" else "",
                style = MaterialTheme.typography.labelMedium
            )
            Spacer(Modifier.height(4.dp))
            when (field.kind) {
                "boolean" -> Row(verticalAlignment =
                    androidx.compose.ui.Alignment.CenterVertically) {
                    Switch(
                        checked = value == "true",
                        onCheckedChange = {
                            values[field.key] = it.toString()
                            onChange(serialize(fields, values))
                        }
                    )
                    Spacer(Modifier.width(10.dp))
                    Text(if (value == "true") "启用" else "关闭",
                        style = MaterialTheme.typography.labelSmall)
                }
                "enum" -> {
                    var expanded by remember(method, field.key) { mutableStateOf(false) }
                    Box {
                        OutlinedButton(onClick = { expanded = true },
                            modifier = Modifier.fillMaxWidth()) {
                            Text(value.ifBlank { "选择 " + field.label },
                                modifier = Modifier.weight(1f))
                            Icon(Icons.Outlined.ExpandMore, null)
                        }
                        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                            field.choices.forEach { choice ->
                                DropdownMenuItem(text = { Text(choice) }, onClick = {
                                    values[field.key] = choice
                                    onChange(serialize(fields, values))
                                    expanded = false
                                })
                            }
                        }
                    }
                }
                else -> OutlinedTextField(
                    value = value,
                    onValueChange = {
                        values[field.key] = it
                        onChange(serialize(fields, values))
                    },
                    placeholder = {
                        Text(when (field.kind) {
                            "object" -> "{ }"
                            "array" -> "[ ]"
                            "integer" -> "0"
                            else -> field.key
                        }, style = MaterialTheme.typography.bodySmall)
                    },
                    modifier = Modifier.fillMaxWidth(),
                    minLines = if (field.kind in listOf("object", "array")) 2 else 1,
                    maxLines = if (field.kind in listOf("object", "array")) 5 else 2,
                    textStyle = MaterialTheme.typography.bodySmall.copy(
                        fontFamily = if (field.kind in listOf("object", "array"))
                            FontFamily.Monospace else FontFamily.Default)
                )
            }
            if (field.description.isNotBlank()) {
                Spacer(Modifier.height(4.dp))
                Text(field.description, style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 3)
            }
        }
    }
}

private fun serialize(fields: List<ParameterField>, values: Map<String, String>): String =
    try { RpcForms.build(fields, values).toString(2) }
    catch (error: Exception) {
        // Reject incomplete forms at the submit boundary instead of inventing values.
        "表单尚未填写正确：" + (error.message ?: "参数错误")
    }
