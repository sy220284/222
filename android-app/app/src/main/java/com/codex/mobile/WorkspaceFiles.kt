package com.codex.mobile

import java.io.File
import java.io.IOException
import java.nio.charset.Charset
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Path
import java.nio.file.SimpleFileVisitor
import java.nio.file.FileVisitResult
import java.nio.file.attribute.BasicFileAttributes

/**
 * Android-private workspace actions; paths are checked at every entrypoint.
 * The file manager never follows a symlink outside the workspace.
 */
internal class WorkspaceFiles(private val workspace: File) {
    private val root = workspace.canonicalFile

    init {
        if (!root.isDirectory && !root.mkdirs()) {
            throw IOException("无法创建工作区：$root")
        }
    }

    fun root(): File = root

    private fun validated(file: File): File {
        val path = file.canonicalFile
        require(path == root || path.path.startsWith(root.path + File.separator)) {
            "操作范围限于手机工作区"
        }
        return path
    }

    private fun child(parent: File, name: String): File {
        require(name.isNotBlank() && name != "." && name != ".." &&
            !name.contains('/') && !name.contains('\\') &&
            !name.contains('\u0000')) { "请输入有效的文件或文件夹名称" }
        val directory = validated(parent)
        require(directory.isDirectory) { "目标文件夹不存在：" + directory.name }
        return validated(File(directory, name.trim()))
    }

    fun list(parent: File = root): List<File> {
        val directory = validated(parent)
        require(directory.isDirectory) { "请选择文件夹" }
        return directory.listFiles()?.sortedWith(
            compareByDescending<File> { it.isDirectory }.thenBy { it.name.lowercase() }
        ) ?: throw IOException("无法读取文件夹：" + directory.name)
    }

    fun relative(file: File): String {
        val path = validated(file)
        return if (path == root) "工作区" else path.relativeTo(root).invariantSeparatorsPath
    }

    fun parent(file: File): File {
        val path = validated(file)
        return if (path == root) root else validated(path.parentFile ?: root)
    }

    fun createFolder(parent: File, name: String): File {
        val target = child(parent, name)
        require(!target.exists()) { "已存在同名文件或文件夹" }
        if (!target.mkdir()) throw IOException("创建文件夹失败：" + target.name)
        return target
    }

    fun createTextFile(parent: File, name: String): File {
        val target = child(parent, name)
        require(!target.exists()) { "已存在同名文件或文件夹" }
        if (!target.createNewFile()) throw IOException("创建文件失败：" + target.name)
        return target
    }

    fun rename(source: File, name: String): File {
        val old = validated(source)
        require(old != root) { "工作区根目录不可重命名" }
        require(old.exists()) { "文件已经不存在" }
        val target = child(parent(old), name)
        require(!target.exists()) { "目标名称已存在" }
        if (!old.renameTo(target)) throw IOException("重命名失败：" + old.name)
        return target
    }

    fun delete(source: File) {
        val file = validated(source)
        require(file != root) { "工作区根目录不可删除" }
        require(file.exists()) { "文件已经不存在" }
        val start = file.toPath()
        Files.walkFileTree(start, object : SimpleFileVisitor<Path>() {
            override fun visitFile(path: Path, attrs: BasicFileAttributes): FileVisitResult {
                val canonical = path.toFile().canonicalFile
                require(canonical == root || canonical.path.startsWith(root.path + File.separator)) {
                    "文件夹中有超出工作区的符号链接"
                }
                Files.delete(path)
                return FileVisitResult.CONTINUE
            }
            override fun postVisitDirectory(path: Path, exception: IOException?): FileVisitResult {
                if (exception != null) throw exception
                Files.delete(path)
                return FileVisitResult.CONTINUE
            }
        })
    }

    fun readText(source: File): String {
        val file = validated(source)
        require(file.isFile) { "请选择普通文件" }
        require(file.length() <= MAX_TEXT_BYTES) {
            "文件超过 2 MB，请使用 Codex 的文件工具处理"
        }
        return file.readText(Charsets.UTF_8)
    }

    fun saveText(source: File, value: String) {
        val file = validated(source)
        require(file.isFile) { "文件已经不存在" }
        require(value.toByteArray(Charsets.UTF_8).size <= MAX_TEXT_BYTES) {
            "文本超过 2 MB，请使用 Codex 的文件工具处理"
        }
        val temp = File(file.parentFile, "." + file.name + ".mobile-writing")
        try {
            temp.writeText(value, Charsets.UTF_8)
            Files.move(temp.toPath(), file.toPath(),
                java.nio.file.StandardCopyOption.REPLACE_EXISTING,
                java.nio.file.StandardCopyOption.ATOMIC_MOVE)
        } finally {
            if (temp.exists()) temp.delete()
        }
    }

    companion object {
        private const val MAX_TEXT_BYTES = 2L * 1024 * 1024
    }
}
