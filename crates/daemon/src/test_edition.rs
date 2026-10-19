//! Test-only storage and removal. Never accepts a caller-supplied deletion path.
use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

fn checked_directory(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                bail!("Refusing a redirected test directory: {}", path.display());
            }
        }
        if meta.is_symlink() || !meta.is_dir() {
            bail!("Refusing a redirected test directory: {}", path.display());
        }
    }
    Ok(())
}

pub fn lock() -> Result<Option<fs::File>> {
    if !crossdesk_core::profile::IS_TEST {
        return Ok(None);
    }
    let dir = crossdesk_transport::data_dir();
    checked_directory(&dir)?;
    fs::create_dir_all(&dir)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".session.lock"))?;
    file.try_lock()
        .context("请先退出 CrossDesk Test，再重试。正式版不需要退出。")?;
    Ok(Some(file))
}

pub fn remove() -> Result<()> {
    if !crossdesk_core::profile::IS_TEST {
        bail!("正式版不提供测试数据清理命令。");
    }
    let dir = crossdesk_transport::data_dir();
    let guard = lock()?;
    let id = crossdesk_core::profile::BUNDLE_ID;
    let mut caches = Vec::new();
    #[cfg(windows)]
    for variable in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(base) = std::env::var_os(variable) {
            caches.push(std::path::PathBuf::from(base).join(id));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let library = std::path::PathBuf::from(std::env::var_os("HOME").context("HOME missing")?)
            .join("Library");
        for parent in ["Application Support", "Caches", "WebKit", "HTTPStorages"] {
            caches.push(library.join(parent).join(id));
        }
        caches.push(
            library
                .join("Saved Application State")
                .join(format!("{id}.savedState")),
        );
    }
    for path in &caches {
        checked_directory(path)?;
    }
    #[cfg(target_os = "macos")]
    {
        // Delete only this edition's service, including orphaned references.
        let mut exhausted = false;
        for _ in 0..256 {
            let result = std::process::Command::new("/usr/bin/security")
                .args([
                    "delete-generic-password",
                    "-s",
                    crossdesk_core::profile::KEYCHAIN_SERVICE,
                ])
                .output()?;
            if result.status.code() == Some(44) {
                exhausted = true;
                break;
            }
            if !result.status.success() {
                bail!(
                    "测试版钥匙串清理失败：{}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }
        }
        if !exhausted {
            bail!("测试版钥匙串条目过多，请重新运行清理工具。");
        }
        // `defaults` removes only the test app's preferences and its cached domain.
        let _ = std::process::Command::new("/usr/bin/defaults")
            .args(["delete", id])
            .output()?;
        for permission in ["Accessibility", "ListenEvent"] {
            let result = std::process::Command::new("/usr/bin/tccutil")
                .args(["reset", permission, id])
                .output()?;
            if !result.status.success() {
                eprintln!("系统未自动移除 {permission} 权限记录；可在系统设置中删除 CrossDesk Test。正式版权限未更改。");
            }
        }
    }
    for path in caches {
        if path.exists() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("请先退出 CrossDesk Test：{}", path.display()))?;
        }
    }
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_name() == ".session.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    drop(guard);
    fs::remove_file(dir.join(".session.lock"))?;
    fs::remove_dir(&dir)?;
    println!(
        "CrossDesk Test 的配对、设置和界面数据已清理。可以删除测试版文件夹；正式版数据未更改。"
    );
    Ok(())
}
