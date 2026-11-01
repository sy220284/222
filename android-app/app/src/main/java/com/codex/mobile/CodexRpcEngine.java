package com.codex.mobile;

import android.content.Context;
import org.json.JSONException;
import org.json.JSONObject;
import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Full duplex App Server transport. All requests use the upstream JSON-RPC wire protocol.
 * The long-lived Rust server owns threads, tools, approvals, attachments and persistence.
 */
public final class CodexRpcEngine implements AutoCloseable {
    public interface Events {
        void onReady();
        void onNotification(JSONObject message);
        void onServerRequest(JSONObject message);
        void onDisconnected(String reason);
    }

    private final Context context;
    private final SecretStore secretStore;
    private final AtomicLong nextId = new AtomicLong(1L);
    private final Map<Long, CompletableFuture<JSONObject>> pending = new ConcurrentHashMap<>();
    private final Object outputLock = new Object();
    private volatile BufferedWriter stdin;
    private volatile Process process;
    private volatile Events events;
    private volatile boolean initialized;

    public CodexRpcEngine(Context context) {
        this.context = context.getApplicationContext();
        this.secretStore = new SecretStore(context);
    }

    public boolean isConnected() { return initialized && process != null && process.isAlive(); }

    public File workspace() throws IOException {
        File location = new File(context.getFilesDir(), "workspace");
        if (!location.isDirectory() && !location.mkdirs()) throw new IOException("无法创建工作区");
        return location;
    }

    private File ensureDirectory(File dir) throws IOException {
        if (!dir.isDirectory() && !dir.mkdirs()) throw new IOException("无法创建目录：" + dir);
        return dir;
    }

    private File binary() throws IOException {
        File binary = new File(context.getApplicationInfo().nativeLibraryDir, "libcodex.so");
        if (!binary.isFile() || !binary.canExecute()) {
            throw new IOException("Codex ARM64 原生核心缺失，APK 尚未完成完整打包");
        }
        try (java.io.FileInputStream input = new java.io.FileInputStream(binary)) {
            byte[] magic = new byte[4];
            if (input.read(magic) != 4 || magic[0] != 0x7f || magic[1] != 'E'
                    || magic[2] != 'L' || magic[3] != 'F') {
                throw new IOException("Codex 原生核心 ELF 校验失败");
            }
        }
        return binary;
    }

    public synchronized void start(Events callback) throws Exception {
        if (process != null && process.isAlive()) throw new IllegalStateException("核心已经启动");
        this.events = callback;
        this.initialized = false;
        File root = ensureDirectory(new File(context.getFilesDir(), "home"));
        File codexHome = ensureDirectory(new File(root, ".codex"));
        File temp = ensureDirectory(new File(context.getCacheDir(), "runtime"));
        File workspace = workspace();
        ProcessBuilder builder = new ProcessBuilder(binary().getAbsolutePath(),
                "app-server", "--listen", "stdio://");
        builder.directory(workspace);
        Map<String, String> env = builder.environment();
        env.put("HOME", root.getAbsolutePath());
        env.put("CODEX_HOME", codexHome.getAbsolutePath());
        env.put("XDG_CONFIG_HOME", root.getAbsolutePath());
        env.put("TMPDIR", temp.getAbsolutePath());
        env.put("PATH", "/system/bin:/system/xbin");
        env.put("RUST_LOG", "error");
        env.remove("OPENAI_API_KEY");
        String apiKey = secretStore.read();
        if (!apiKey.isEmpty()) env.put("OPENAI_API_KEY", apiKey);
        process = builder.start();
        Process running = process;
        stdin = new BufferedWriter(new OutputStreamWriter(running.getOutputStream(), StandardCharsets.UTF_8));
        Thread errors = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(
                    new InputStreamReader(running.getErrorStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    android.util.Log.w("CodexRuntime", apiKey.isEmpty() ? line : line.replace(apiKey, "[REDACTED]"));
                }
            } catch (IOException exception) {
                android.util.Log.e("CodexRuntime", "stderr 读取异常", exception);
            }
        }, "codex-diagnostics");
        errors.setDaemon(true);
        errors.start();
        Thread reader = new Thread(() -> readLoop(running), "codex-json-rpc");
        reader.setDaemon(true);
        reader.start();

        JSONObject info = new JSONObject()
                .put("name", "codex-mobile")
                .put("title", "Codex Android")
                .put("version", "0.2.0");
        JSONObject capabilities = new JSONObject()
                .put("experimentalApi", true)
                .put("requiredHumanApproval", true)
                .put("mcpServerOpenaiFormElicitation", true);
        JSONObject params = new JSONObject().put("clientInfo", info).put("capabilities", capabilities);
        request("initialize", params).whenComplete((result, failure) -> {
            if (failure != null) {
                notifyDisconnect("核心初始化失败：" + failure.getMessage());
                stop();
                return;
            }
            try {
                notifyMethod("initialized", null);
                initialized = true;
                callback.onReady();
            } catch (Exception error) {
                notifyDisconnect("初始化握手失败：" + error.getMessage());
                stop();
            }
        });
    }

    private void readLoop(Process running) {
        String failure = "Codex 原生服务已停止";
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                running.getInputStream(), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                JSONObject message;
                try {
                    message = new JSONObject(line);
                } catch (JSONException error) {
                    throw new IOException("Codex 输出了非 JSON-RPC 数据：" + line, error);
                }
                if (message.has("id") && (message.has("result") || message.has("error"))) {
                    long id = message.getLong("id");
                    CompletableFuture<JSONObject> future = pending.remove(id);
                    if (future != null) {
                        if (message.has("error")) {
                            future.completeExceptionally(new IOException(message.getJSONObject("error").toString()));
                        } else {
                            future.complete(message.optJSONObject("result") != null
                                    ? message.getJSONObject("result")
                                    : new JSONObject().put("value", message.opt("result")));
                        }
                    }
                } else if (message.has("id") && message.has("method")) {
                    Events sink = events;
                    if (sink != null) sink.onServerRequest(message);
                } else if (message.has("method")) {
                    Events sink = events;
                    if (sink != null) sink.onNotification(message);
                }
            }
            failure = "Codex 进程已退出（代码 " + running.waitFor() + "）";
        } catch (Exception error) {
            failure = "Codex 通道异常：" + error.getMessage();
        } finally {
            initialized = false;
            if (process == running) process = null;
            stdin = null;
            for (CompletableFuture<JSONObject> future : pending.values()) {
                future.completeExceptionally(new IOException(failure));
            }
            pending.clear();
            notifyDisconnect(failure);
        }
    }

    public CompletableFuture<JSONObject> request(String method, JSONObject params) {
        CompletableFuture<JSONObject> response = new CompletableFuture<>();
        long id = nextId.getAndIncrement();
        JSONObject envelope = new JSONObject();
        try {
            envelope.put("id", id);
            envelope.put("method", method);
            if (params != null) envelope.put("params", params);
            pending.put(id, response);
            send(envelope);
        } catch (Exception error) {
            pending.remove(id);
            response.completeExceptionally(error);
        }
        return response;
    }

    public void notifyMethod(String method, JSONObject params) throws Exception {
        JSONObject envelope = new JSONObject().put("method", method);
        if (params != null) envelope.put("params", params);
        send(envelope);
    }

    public void respond(Object id, JSONObject result) throws Exception {
        send(new JSONObject().put("id", id).put("result", result));
    }

    public void reject(Object id, String reason) throws Exception {
        send(new JSONObject().put("id", id).put("error",
                new JSONObject().put("code", -32000).put("message", reason)));
    }

    private void send(JSONObject message) throws IOException {
        synchronized (outputLock) {
            if (stdin == null) throw new IOException("Codex 连接尚未建立或已经断开");
            stdin.write(message.toString());
            stdin.write('\n');
            stdin.flush();
        }
    }

    private void notifyDisconnect(String text) {
        Events callback = events;
        if (callback != null) callback.onDisconnected(text);
    }

    public synchronized void stop() {
        initialized = false;
        Process current = process;
        if (current != null) {
            current.destroy();
            if (current.isAlive()) current.destroyForcibly();
        }
    }

    @Override public void close() { stop(); }
}
