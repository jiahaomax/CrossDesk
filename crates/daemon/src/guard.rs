//! Injection ownership lives in a child process. If the network daemon dies, EOF/lease expiry releases inputs.
//! The private inherited pipe is never exposed to the WebView or the LAN.
use anyhow::{bail, Context, Result};
use crossdesk_core::{receiver::Receiver, InputEvent};
use crossdesk_platform as platform;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Receiver as Replies},
    time::{Duration, Instant},
};
#[derive(Serialize, Deserialize)]
enum Request {
    Inject(InputEvent),
    Pulse,
    Quit,
}
pub struct Guard {
    child: Child,
    input: Option<ChildStdin>,
    replies: Replies<bool>,
}
const ACK_TIMEOUT: Duration = Duration::from_millis(150);

// One small request is outstanding at a time. A reader thread lets the owner
// bound its acknowledgement wait and close stdin (EOF) on any failure.
fn read_replies(output: ChildStdout) -> Replies<bool> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut output = BufReader::new(output);
        loop {
            let mut answer = String::new();
            let ok = output.by_ref().take(128).read_line(&mut answer).is_ok() && answer == "ok\n";
            if tx.send(ok).is_err() || !ok {
                break;
            }
        }
    });
    rx
}
fn await_reply(replies: &Replies<bool>, timeout: Duration) -> Result<()> {
    match replies.recv_timeout(timeout) {
        Ok(true) => Ok(()),
        _ => bail!("native injection guardian failed, stopped or acknowledgement timed out"),
    }
}
impl Guard {
    pub fn spawn(lease_ms: u64) -> Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--input-guard")
            .arg(lease_ms.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let input = child.stdin.take().context("guard input pipe")?;
        let output = child.stdout.take().context("guard output pipe")?;
        Ok(Self {
            child,
            input: Some(input),
            replies: read_replies(output),
        })
    }
    fn request(&mut self, request: Request) -> Result<()> {
        let mut data = serde_json::to_vec(&request)?;
        data.push(b'\n');
        let result = (|| {
            let input = self.input.as_mut().context("guardian already stopped")?;
            input.write_all(&data)?;
            input.flush()?;
            await_reply(&self.replies, ACK_TIMEOUT)
        })();
        if result.is_err() {
            // A late acknowledgement must never authorize a later injection.
            // EOF also asks a responsive guardian to release its own ledger.
            self.input.take();
        }
        result
    }
    pub fn inject(&mut self, event: &InputEvent) -> Result<()> {
        self.request(Request::Inject(event.clone()))
    }
    pub fn pulse(&mut self) -> Result<()> {
        self.request(Request::Pulse)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(b"\"Quit\"\n");
            let _ = input.flush();
        }
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(100) {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // Do not kill the last owner of the input ledger. Its own bounded lease remains responsible for cleanup.
        crate::native::mark_unconfirmed_cleanup();
    }
}
struct OwnedInputs(Receiver);
impl OwnedInputs {
    fn release(&mut self) -> Result<()> {
        let mut failed = false;
        for e in self.0.release() {
            if platform::inject(&e).is_err() {
                failed = true;
                match e {
                    InputEvent::Key { key, .. } => self.0.committed(&InputEvent::Key {
                        key,
                        down: true,
                        repeat: false,
                    }),
                    InputEvent::Button { button, .. } => {
                        self.0.committed(&InputEvent::Button { button, down: true })
                    }
                    _ => {}
                }
            }
        }
        if failed {
            bail!("native release refused by OS");
        }
        Ok(())
    }
}
impl Drop for OwnedInputs {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
pub fn run(lease_ms: u64) -> Result<()> {
    if !(500..=2000).contains(&lease_ms) {
        bail!("invalid guardian lease");
    }
    platform::dpi_awareness()?;
    let (tx, rx) = std::sync::mpsc::sync_channel(32);
    std::thread::spawn(move || {
        let input = std::io::stdin();
        let mut r = input.lock();
        loop {
            let mut line = String::new();
            match r.by_ref().take(4097).read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.len() > 4096 {
                        break;
                    }
                    let Ok(request) = serde_json::from_str::<Request>(&line) else {
                        break;
                    };
                    if tx.send(request).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut owned = OwnedInputs(Receiver::default());
    let mut last = Instant::now();
    #[cfg(windows)]
    let mut desktop_check = Instant::now();
    loop {
        if last.elapsed() > Duration::from_millis(lease_ms) {
            break; // Expiry is terminal; a delayed Pulse cannot revive the old guardian.
        }
        #[cfg(windows)]
        if desktop_check.elapsed() >= Duration::from_millis(50) {
            desktop_check = Instant::now();
            if !platform::input_desktop_available() {
                break;
            }
        }
        let request = match rx.recv_timeout(Duration::from_millis(8)) {
            Ok(r) => r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        let result = match request {
            Request::Quit => break,
            Request::Pulse => {
                last = Instant::now();
                Ok(())
            }
            Request::Inject(event) => {
                let orphan = match &event {
                    InputEvent::Key {
                        key, down: false, ..
                    } => !owned.0.keys.contains(key),
                    InputEvent::Button {
                        button,
                        down: false,
                    } => !owned.0.buttons.contains(button),
                    _ => false,
                };
                if orphan {
                    Ok(())
                } else {
                    platform::inject(&event).map(|_| owned.0.committed(&event))
                }
            }
        };
        if result.is_err() {
            let _ = owned.release();
        }
        println!("{}", if result.is_ok() { "ok" } else { "failed" });
        std::io::stdout().flush()?;
    }
    owned.release()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_guard_ack_is_bounded_without_native_input() {
        let (_tx, rx) = mpsc::sync_channel(1);
        assert!(await_reply(&rx, Duration::from_millis(5)).is_err());
    }
    #[test]
    fn only_successful_guard_ack_is_accepted() {
        let (tx, rx) = mpsc::sync_channel(1);
        tx.send(true).unwrap();
        assert!(await_reply(&rx, Duration::from_millis(5)).is_ok());
        tx.send(false).unwrap();
        assert!(await_reply(&rx, Duration::from_millis(5)).is_err());
        drop(tx);
        assert!(await_reply(&rx, Duration::from_millis(5)).is_err());
    }
}
