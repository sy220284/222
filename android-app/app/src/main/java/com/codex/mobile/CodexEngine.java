package com.codex.mobile;

import android.content.Context;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class CodexEngine {
    interface Callback {
        void onEvent(String line);
        void onFinished(int exitCode, String errors);
    }

    private final Context context;
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private volatile Process process;
    private volatile boolean running = false;

    CodexEngine(Context applicationContext) {
        this.context = applicationContext;
    }

    boolean isRunning() { return running; }

    File workspace() {
        File workspace = new File(context.getFilesDir(), "workspace");
        if (!workspace.isDirectory() && !workspace.mkdirs()) {
            throw new IllegalStateException("无法创建独立代码工作区");
        }
        return workspace;
    }

    private File executable() throws IOException {
        File binary = new File(context.getApplicationInfo().nativeLibraryDir, "libcodex.so");
        if (!binary.isFile() || !binary.canExecute()) {
            throw new IOException("APK 缺少 Android ARM64 的原生 Codex 执行文件");
        }
        try (FileInputStream input = new FileInputStream(binary)) {
            byte[] elf = new byte[4];
            if (input.read(elf) != 4 || elf[0] != 0x7f || elf[1] != 'E'
                    || elf[2] != 'L' || elf[3] != 'F') {
                throw new IOException("打包的 Codex 执行文件不是有效的 ELF");
            }
        }
        return binary;
    }

    synchronized void execute(String prompt, String threadId, String model,
                              boolean readOnly, String apiKey, Callback callback)
            throws IOException {
        if (running) throw new IllegalStateException("已有 Codex 任务正在执行");
        File binary = executable();
        File workDir = workspace();
        File home = new File(context.getFilesDir(), "home");
        File codexHome = new File(home, ".codex");
        File cache = new File(context.getCacheDir(), "codex-tmp");
        if ((!home.isDirectory() && !home.mkdirs())
                || (!codexHome.isDirectory() && !codexHome.mkdirs())
                || (!cache.isDirectory() && !cache.mkdirs())) {
            throw new IOException("无法创建 Codex 私有运行目录");
        }

        ArrayList<String> command = new ArrayList<>();
        command.add(binary.getAbsolutePath());
        command.add("exec");
        command.add("--json");
        command.add("--skip-git-repo-check");
        command.add("--sandbox");
        command.add(readOnly ? "read-only" : "workspace-write");
        command.add("-C");
        command.add(workDir.getAbsolutePath());
        if (model != null && !model.trim().isEmpty()) {
            command.add("--model");
            command.add(model.trim());
        }
        if (threadId != null && !threadId.isEmpty()) {
            command.add("resume");
            command.add(threadId);
        }
        // Keep the prompt out of the process command line.
        command.add("-");
        ProcessBuilder builder = new ProcessBuilder(command).directory(workDir);
        Map<String, String> env = builder.environment();
        env.put("HOME", home.getAbsolutePath());
        env.put("CODEX_HOME", codexHome.getAbsolutePath());
        env.put("XDG_CONFIG_HOME", home.getAbsolutePath());
        env.put("TMPDIR", cache.getAbsolutePath());
        env.put("RUST_LOG", "error");
        env.put("PATH", "/system/bin:/system/xbin");
        env.remove("OPENAI_API_KEY");
        if (apiKey != null && !apiKey.isEmpty()) env.put("OPENAI_API_KEY", apiKey);

        process = builder.start();
        running = true;
        Process active = process;
        executor.execute(() -> {
            StringBuilder stderr = new StringBuilder();
            Thread stderrReader = new Thread(() -> {
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                        active.getErrorStream(), StandardCharsets.UTF_8))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        // Preserve errors, including diagnostics needed for support.
                        synchronized (stderr) {
                            if (stderr.length() < 50000) stderr.append(line).append('\n');
                        }
                    }
                } catch (IOException error) {
                    synchronized (stderr) {
                        stderr.append("stderr 读取异常：").append(error.getMessage()).append('\n');
                    }
                }
            }, "codex-stderr");
            stderrReader.start();
            int code = -1;
            try {
                try (BufferedWriter stdin = new BufferedWriter(new OutputStreamWriter(
                        active.getOutputStream(), StandardCharsets.UTF_8))) {
                    stdin.write(prompt);
                    stdin.newLine();
                }
                try (BufferedReader stdout = new BufferedReader(new InputStreamReader(
                        active.getInputStream(), StandardCharsets.UTF_8))) {
                    String line;
                    while ((line = stdout.readLine()) != null) callback.onEvent(line);
                }
                code = active.waitFor();
                stderrReader.join();
            } catch (Exception error) {
                synchronized (stderr) {
                    stderr.append("运行异常：").append(error.getMessage()).append('\n');
                }
                active.destroyForcibly();
                try { stderrReader.join(1000); } catch (InterruptedException interrupted) {
                    Thread.currentThread().interrupt();
                }
            } finally {
                synchronized (CodexEngine.this) {
                    if (process == active) {
                        process = null;
                        running = false;
                    }
                }
                callback.onFinished(code, stderr.toString());
            }
        });
    }

    void cancel() {
        Process current = process;
        if (current != null) {
            current.destroy();
            if (current.isAlive()) current.destroyForcibly();
        }
    }

    void close() {
        cancel();
        executor.shutdownNow();
    }
}
