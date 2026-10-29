//! Native atlas-cli:// operating system registration based on
//! 2.1.88 src/utils/deepLink/registerProtocol.ts.
//! Registration is explicit and installation-local, not telemetry-controlled.
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const SCHEME: &str = "atlas-cli";

fn desktop_file() -> Result<PathBuf> {
    let dir = if let Some(user) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(user)
    } else {
        dirs::home_dir()
            .context("Home directory unavailable")?
            .join(".local/share")
    };
    Ok(dir.join("applications/atlas-agent-url-handler.desktop"))
}

fn linux_exec_line(executable: &Path) -> Result<String> {
    let path = executable.to_str().context("Non-UTF-8 binary path")?;
    if path.contains(['\n', '\r', '"', '%']) {
        bail!("Executable path cannot be represented in a desktop Exec field");
    }
    Ok(format!("Exec=\"{path}\" handle-uri %u"))
}

#[cfg(target_os = "linux")]
fn register_linux(executable: &Path) -> Result<()> {
    let path = desktop_file()?;
    fs::create_dir_all(path.parent().context("Invalid desktop path")?)?;
    let line = linux_exec_line(executable)?;
    let contents = format!(
        "[Desktop Entry]\nName=Atlas Agent URL Handler\nComment=Open atlas-cli:// links in Atlas Agent\n{line}\nType=Application\nNoDisplay=true\nMimeType=x-scheme-handler/{SCHEME};\n"
    );
    fs::write(&path, contents)?;
    if let Ok(status) = Command::new("xdg-mime")
        .args([
            "default",
            "atlas-agent-url-handler.desktop",
            "x-scheme-handler/atlas-cli",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        if !status.success() {
            bail!("xdg-mime could not register atlas-cli URL scheme");
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_current_linux(executable: &Path) -> bool {
    let Ok(path) = desktop_file() else {
        return false;
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|content| linux_exec_line(executable)
            .ok().map(|line| content.contains(&line)))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn register_windows(executable: &Path) -> Result<()> {
    let exe = executable.to_string_lossy();
    let key = format!(r"HKCU\Software\Classes\{SCHEME}");
    let command = format!("{key}\\shell\\open\\command");
    let value = format!("\"{exe}\" handle-uri \"%1\"");
    for args in [
        vec!["add", &key, "/ve", "/d", "URL:Atlas Agent URL Handler", "/f"],
        vec!["add", &key, "/v", "URL Protocol", "/d", "", "/f"],
        vec!["add", &command, "/ve", "/d", &value, "/f"],
    ] {
        let status = Command::new("reg")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            bail!("Failed to register atlas-cli in Windows registry");
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_current_windows(executable: &Path) -> bool {
    let key = format!(r"HKCU\Software\Classes\{SCHEME}\shell\open\command");
    let value = format!("\"{}\" handle-uri \"%1\"", executable.display());
    Command::new("reg")
        .args(["query", &key, "/ve"])
        .output()
        .ok()
        .filter(|result| result.status.success())
        .is_some_and(|result| String::from_utf8_lossy(&result.stdout).contains(&value))
}

#[cfg(target_os = "macos")]
fn macos_app_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("Home directory unavailable")?
        .join("Applications/Atlas Agent URL Handler.app"))
}

#[cfg(target_os = "macos")]
fn register_macos(executable: &Path) -> Result<()> {
    let app = macos_app_dir()?;
    let path = executable.to_str().context("Non-UTF-8 executable path")?;
    if path.contains(['\n','\r']) {
        bail!("Invalid executable path for AppleScript");
    }
    let script = format!(
        "on open location address\n do shell script quoted form of {executable} & \" handle-uri \" & quoted form of address & \" >/dev/null 2>&1 &\"\nend open location\n",
        executable = applescript_string(path)
    );
    let temp = std::env::temp_dir().join(format!("atlas-register-{}.applescript", uuid::Uuid::new_v4()));
    fs::write(&temp, script)?;
    if app.exists() {
        fs::remove_dir_all(&app)?;
    }
    fs::create_dir_all(app.parent().context("Invalid app path")?)?;
    let compiled = Command::new("osacompile")
        .arg("-o")
        .arg(&app)
        .arg(&temp)
        .status()?;
    let _ = fs::remove_file(&temp);
    if !compiled.success() {
        bail!("Could not compile AppleScript URL handler");
    }
    let plist = app.join("Contents/Info.plist");
    let content = fs::read_to_string(&plist)?;
    let url_types = format!(
        "<key>CFBundleIdentifier</key><string>dev.atlas-agent.urlhandler</string>\n\
         <key>CFBundleURLTypes</key><array><dict>\
         <key>CFBundleURLName</key><string>Atlas Agent Deep Link</string>\
         <key>CFBundleURLSchemes</key><array><string>{SCHEME}</string></array>\
         </dict></array>\n"
    );
    let rewritten = content.replacen("</dict>", &format!("{url_types}</dict>"), 1);
    fs::write(plist, rewritten)?;
    let register = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = Command::new(register).args(["-R", &app.to_string_lossy()]).status();
    // Commit marker is written only after the app was compiled and registered.
    fs::write(app.join("Contents/Resources/atlas-agent-executable"), path)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn applescript_string(input: &str) -> String {
    format!("\"{}\"", input.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "macos")]
fn is_current_macos(executable: &Path) -> bool {
    let Ok(app) = macos_app_dir() else {
        return false;
    };
    fs::read_to_string(app.join("Contents/Resources/atlas-agent-executable"))
        .ok()
        .is_some_and(|old| old == executable.to_string_lossy())
}

pub fn is_protocol_handler_current(executable: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        is_current_linux(executable)
    }
    #[cfg(target_os = "windows")]
    {
        is_current_windows(executable)
    }
    #[cfg(target_os = "macos")]
    {
        is_current_macos(executable)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = executable;
        false
    }
}

pub fn register_protocol_handler(executable: &Path) -> Result<()> {
    if !executable.is_absolute() || !executable.is_file() {
        bail!("Protocol handler requires the absolute path of an installed executable");
    }
    #[cfg(target_os = "linux")]
    {
        register_linux(executable)
    }
    #[cfg(target_os = "windows")]
    {
        register_windows(executable)
    }
    #[cfg(target_os = "macos")]
    {
        register_macos(executable)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        bail!("Operating system does not support protocol registration")
    }
}

pub fn ensure_protocol_handler_current(executable: &Path) -> Result<bool> {
    if is_protocol_handler_current(executable) {
        return Ok(false);
    }
    register_protocol_handler(executable)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_artifact_matches_writer_and_rejects_invalid_binary_path() {
        let binary = Path::new("/home/user/Atlas Agent/atlas-agent");
        let exec = linux_exec_line(binary).unwrap();
        assert_eq!(exec, "Exec=\"/home/user/Atlas Agent/atlas-agent\" handle-uri %u");
        assert!(linux_exec_line(Path::new("/tmp/bad\"name")).is_err());
        assert!(linux_exec_line(Path::new("/tmp/new\nline")).is_err());
    }
}
