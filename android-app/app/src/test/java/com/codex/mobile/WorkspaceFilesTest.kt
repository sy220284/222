package com.codex.mobile

import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.nio.file.Files

class WorkspaceFilesTest {
    @get:Rule val temp = TemporaryFolder()

    @Test fun createsEditsRenamesAndDeletesFiles() {
        val root = temp.newFolder("workspace")
        val manager = WorkspaceFiles(root)
        val folder = manager.createFolder(root, "项目")
        val file = manager.createTextFile(folder, "README.md")
        manager.saveText(file, "# 示例项目")
        assertEquals("# 示例项目", manager.readText(file))
        assertEquals("项目/README.md", manager.relative(file))
        val renamed = manager.rename(file, "新名字.md")
        assertFalse(file.exists())
        assertEquals("# 示例项目", manager.readText(renamed))
        assertEquals(listOf("新名字.md"), manager.list(folder).map { it.name })
        manager.delete(folder)
        assertFalse(folder.exists())
        assertEquals(emptyList<File>(), manager.list(root))
    }

    @Test fun preventsLeavingPrivateWorkspace() {
        val root = temp.newFolder("project")
        val outside = temp.newFolder("outside")
        val manager = WorkspaceFiles(root)
        expectFailure { manager.readText(File(outside, "private.txt")) }
        expectFailure { manager.delete(outside) }
        expectFailure { manager.createFolder(root, "../escape") }
        expectFailure { manager.createFolder(root, "/absolute") }
        expectFailure { manager.rename(root, "new-root") }
    }

    @Test fun refusesSymlinkTraversal() {
        val root = temp.newFolder("working")
        val outside = temp.newFolder("outside-root")
        val manager = WorkspaceFiles(root)
        val link = root.toPath().resolve("external")
        Files.createSymbolicLink(link, outside.toPath())
        expectFailure { manager.list(link.toFile()) }
        expectFailure { manager.delete(link.toFile()) }
        assertTrue(Files.exists(outside.toPath()))
    }

    @Test fun doesNotOverwriteExistingFiles() {
        val root = temp.newFolder("working")
        val manager = WorkspaceFiles(root)
        val existing = manager.createTextFile(root, "a.txt")
        manager.saveText(existing, "original")
        expectFailure { manager.createTextFile(root, "a.txt") }
        expectFailure { manager.rename(existing, "a.txt") }
        assertEquals("original", manager.readText(existing))
    }

    private inline fun expectFailure(block: () -> Unit) {
        try {
            block()
            fail("Expected file operation to be rejected")
        } catch (expected: IllegalArgumentException) {
            // Explicit path validation failure.
        }
    }
}
