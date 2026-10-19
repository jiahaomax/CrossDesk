#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};
const TEST_EDITION: bool = cfg!(feature = "test-profile");
const APP_NAME: &str = if TEST_EDITION {
    "CrossDesk Test"
} else {
    "CrossDesk"
};
const APP_ID: &str = if TEST_EDITION {
    "dev.crossdesk.desktop.test"
} else {
    "dev.crossdesk.desktop"
};
struct Backend {
    input: Mutex<ChildStdin>,
    child: Mutex<Child>,
    status: Arc<Mutex<Value>>,
    error: Arc<Mutex<String>>,
}
impl Backend {
    fn read_status(&self) -> Result<Value, String> {
        let status = self.status.lock().unwrap().clone();
        if self
            .child
            .lock()
            .unwrap()
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err(format!(
                "原生后台已停止，缓存状态不再有效。请完全退出 CrossDesk 后重新打开。详情：{}",
                self.error.lock().unwrap()
            ));
        }
        Ok(status)
    }

    fn send(&self, message: Value) -> Result<(), String> {
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .ok_or("missing command")?;
        const COMMANDS: &[&str] = &[
            "status",
            "configure",
            "trust",
            "revoke",
            "resume",
            "start_source",
            "start_receiver",
            "pause",
            "quit",
            "refresh",
            "discover",
            "pair_open",
            "pair_connect",
            "pair_confirm",
            "pair_cancel",
            "export_diagnostics",
        ];
        if !COMMANDS.contains(&command) {
            return Err("unknown command".into());
        }
        let mut text = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        if text.len() > 262144 {
            return Err("command too large".into());
        }
        text.push(b'\n');
        let mut stdin = self.input.lock().map_err(|e| e.to_string())?;
        stdin
            .write_all(&text)
            .and_then(|_| stdin.flush())
            .map_err(|e| e.to_string())
    }
    fn stop(&self) {
        let _ = self.send(serde_json::json!({"command":"quit"}));
        let mut child = self.child.lock().unwrap();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(4) {
            if child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}
#[tauri::command]
fn daemon_status(backend: State<Backend>) -> Result<Value, String> {
    backend.read_status()
}
#[tauri::command]
async fn daemon_command(message: Value, backend: State<'_, Backend>) -> Result<(), String> {
    backend.send(message)
}
#[tauri::command]
fn export_pairing(app: tauri::AppHandle, backend: State<Backend>) -> Result<String, String> {
    let status = backend.status.lock().map_err(|e| e.to_string())?.clone();
    let identity = status
        .get("identity")
        .and_then(Value::as_str)
        .ok_or("原生后台尚未就绪")?;
    if identity.len() != 64 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("本机证书指纹无效".into());
    }
    let path = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(format!(
            "{APP_NAME}-{}-public-pairing.json",
            &identity[..12]
        ));
    let public = serde_json::json!({
        "name": status.pointer("/topology/0/device/name"),
        "certificate": status.get("certificate"),
        "fingerprint": identity,
    });
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&public).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("无法写入下载文件夹：{e}"))?;
    Ok(path.to_string_lossy().into_owned())
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--profile-info") {
        println!(
            "{}",
            serde_json::json!({"test":TEST_EDITION,"bundle_id":APP_ID})
        );
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("CrossDesk {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            if app.config().identifier != APP_ID {
                return Err(
                    "Application edition/configuration mismatch; no backend started".into(),
                );
            }
            let exe = std::env::current_exe()?;
            let name = if TEST_EDITION && cfg!(windows) {
                "crossdesk-test-daemon.exe"
            } else if TEST_EDITION {
                "crossdesk-test-daemon"
            } else if cfg!(windows) {
                "crossdesk-daemon.exe"
            } else {
                "crossdesk-daemon"
            };
            let mut path = exe.parent().unwrap().join(name);
            if !path.exists() {
                path = app.path().resource_dir()?.join(name);
            }
            let mut probe = Command::new(&path);
            probe.arg("--profile-info");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                probe.creation_flags(0x08000000);
            }
            let output = probe.output()?;
            let profile: Value = serde_json::from_slice(&output.stdout)?;
            if !output.status.success()
                || profile.get("test").and_then(Value::as_bool) != Some(TEST_EDITION)
            {
                return Err("Desktop/backend edition mismatch; no backend started".into());
            }
            let mut command = Command::new(path);
            command
                .arg("--stdio")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let mut child = command.spawn()?;
            let input = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            let error = Arc::new(Mutex::new(String::new()));
            let errors = error.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    let mut e = errors.lock().unwrap();
                    if e.len() < 4096 {
                        e.push_str(&line);
                        e.push('\n');
                    }
                }
            });
            let status = Arc::new(Mutex::new(Value::Null));
            let output = status.clone();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        if let Some(tray) = handle.tray_by_id("crossdesk") {
                            let label =
                                if value.get("enabled").and_then(Value::as_bool) == Some(true) {
                                    "共享已启用"
                                } else {
                                    "已暂停"
                                };
                            let _ = tray.set_tooltip(Some(format!("{APP_NAME} · {label}")));
                        }
                        *output.lock().unwrap() = value;
                    }
                }
                if let Some(tray) = handle.tray_by_id("crossdesk") {
                    let _ = tray.set_tooltip(Some(format!("{APP_NAME} · 后台已停止")));
                }
                let mut s = output.lock().unwrap();
                if let Some(o) = s.as_object_mut() {
                    o.insert(
                        "state".into(),
                        Value::String("后台已退出，输入共享已停止".into()),
                    );
                    o.insert("active".into(), Value::Null);
                    o.insert("enabled".into(), Value::Bool(false));
                }
            });
            app.manage(Backend {
                input: Mutex::new(input),
                child: Mutex::new(child),
                status,
                error,
            });
            let show = MenuItem::with_id(app, "show", "打开设置", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "暂停并释放输入", true, None::<&str>)?;
            let resume = MenuItem::with_id(app, "resume", "恢复双向共享", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出并停止共享", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &resume, &quit])?;
            TrayIconBuilder::with_id("crossdesk")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(APP_NAME)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "pause" => {
                        let _ = app
                            .state::<Backend>()
                            .send(serde_json::json!({"command":"pause"}));
                    }
                    "resume" => {
                        let backend = app.state::<Backend>();
                        let _ = backend.send(serde_json::json!({"command":"resume"}));
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            daemon_status,
            daemon_command,
            export_pairing,
            quit_app
        ])
        .build(tauri::generate_context!())
        .expect("CrossDesk desktop startup failed");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<Backend>().stop();
        }
    });
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    #[test]
    fn stopped_process_cannot_return_cached_online_devices() {
        for cached in [
            Value::Null,
            serde_json::json!({"enabled":true,"topology":[{"online":true}]}),
        ] {
            let mut child = Command::new("cmd.exe")
                .args(["/C", "exit", "0"])
                .creation_flags(0x08000000)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let input = child.stdin.take().unwrap();
            child.wait().unwrap();
            let backend = Backend {
                input: Mutex::new(input),
                child: Mutex::new(child),
                status: Arc::new(Mutex::new(cached)),
                error: Arc::new(Mutex::new(String::new())),
            };
            assert!(backend
                .read_status()
                .unwrap_err()
                .contains("缓存状态不再有效"));
        }
    }
}
