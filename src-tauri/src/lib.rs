use std::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
#[cfg(debug_assertions)]
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use tauri::Manager;
#[cfg(not(debug_assertions))]
use tauri_plugin_shell::{ShellExt, process::CommandChild};

struct BackendState {
    url: String,
    child: Mutex<Option<BackendProcess>>,
}

enum BackendProcess {
    #[cfg(debug_assertions)]
    Std(Child),
    #[cfg(not(debug_assertions))]
    Sidecar(CommandChild),
}

impl Drop for BackendState {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.take() {
                child.kill();
            }
        }
    }
}

impl BackendProcess {
    fn kill(self) {
        match self {
            #[cfg(debug_assertions)]
            Self::Std(mut child) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            #[cfg(not(debug_assertions))]
            Self::Sidecar(child) => {
                let _ = child.kill();
            }
        }
    }

    fn try_wait_status(&mut self) -> std::io::Result<Option<String>> {
        match self {
            #[cfg(debug_assertions)]
            Self::Std(child) => Ok(child.try_wait()?.map(|status| status.to_string())),
            #[cfg(not(debug_assertions))]
            Self::Sidecar(_) => Ok(None),
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
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![get_backend_url])
        .setup(|app| {
            let port = find_free_port()?;
            let url = format!("http://127.0.0.1:{port}");
            let child = spawn_backend(app, port)?;
            let child = wait_for_backend(port, child)?;
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

#[cfg(debug_assertions)]
fn spawn_backend(_app: &tauri::App, port: u16) -> std::io::Result<BackendProcess> {
    let port = port.to_string();
    let mut command = debug_backend_command();
    command
        .args(["--host", "127.0.0.1", "--port", &port])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    command.spawn().map(BackendProcess::Std)
}

#[cfg(not(debug_assertions))]
fn spawn_backend(app: &tauri::App, port: u16) -> std::io::Result<BackendProcess> {
    let port = port.to_string();
    let (_events, child) = app
        .shell()
        .sidecar("tutor-web")
        .map_err(io_error)?
        .args(["--host", "127.0.0.1", "--port", &port])
        .spawn()
        .map_err(io_error)?;

    Ok(BackendProcess::Sidecar(child))
}

fn wait_for_backend(port: u16, mut child: BackendProcess) -> std::io::Result<BackendProcess> {
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(child);
        }

        if let Some(status) = child.try_wait_status()? {
            return Err(std::io::Error::other(format!(
                "tutor-web exited before it became ready: {status}"
            )));
        }

        if Instant::now() >= deadline {
            child.kill();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for tutor-web to start",
            ));
        }

        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(debug_assertions)]
fn debug_backend_command() -> Command {
    if let Ok(path) = std::env::var("LLM_TUTOR_BACKEND_BIN") {
        return Command::new(path);
    }

    let mut command = Command::new("cargo");
    command.args(["run", "-p", "tutor-web", "--"]);
    if let Some(root) = workspace_root() {
        command.current_dir(root);
    }
    command
}

#[cfg(debug_assertions)]
fn workspace_root() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
}

#[cfg(not(debug_assertions))]
fn io_error(error: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
    std::io::Error::other(error)
}
