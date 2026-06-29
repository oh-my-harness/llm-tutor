use std::{
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::Manager;

struct BackendState {
    url: String,
    child: Mutex<Option<Child>>,
}

impl Drop for BackendState {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut child) = child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[tauri::command]
fn get_backend_url(state: tauri::State<'_, BackendState>) -> String {
    state.url.clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_backend_url])
        .setup(|app| {
            let port = find_free_port()?;
            let url = format!("http://127.0.0.1:{port}");
            let mut child = spawn_backend(port)?;
            wait_for_backend(port, &mut child)?;
            app.manage(BackendState {
                url,
                child: Mutex::new(Some(child)),
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run llm-tutor desktop app");
}

fn find_free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn spawn_backend(port: u16) -> std::io::Result<Child> {
    let port = port.to_string();
    let mut command = backend_command();
    command
        .args(["--host", "127.0.0.1", "--port", &port])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    command.spawn()
}

fn wait_for_backend(port: u16, child: &mut Child) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }

        if let Some(status) = child.try_wait()? {
            return Err(std::io::Error::other(format!(
                "tutor-web exited before it became ready: {status}"
            )));
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for tutor-web to start",
            ));
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn backend_command() -> Command {
    if let Ok(path) = std::env::var("LLM_TUTOR_BACKEND_BIN") {
        return Command::new(path);
    }

    #[cfg(debug_assertions)]
    {
        let mut command = Command::new("cargo");
        command.args(["run", "-p", "tutor-web", "--"]);
        if let Some(root) = workspace_root() {
            command.current_dir(root);
        }
        command
    }

    #[cfg(not(debug_assertions))]
    {
        let exe_name = if cfg!(windows) {
            "tutor-web.exe"
        } else {
            "tutor-web"
        };
        let path = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join(exe_name)))
            .unwrap_or_else(|| PathBuf::from(exe_name));
        Command::new(path)
    }
}

#[cfg(debug_assertions)]
fn workspace_root() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
}
