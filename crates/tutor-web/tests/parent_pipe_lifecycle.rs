use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn exits_when_desktop_parent_pipe_closes() {
    let data_dir =
        std::env::temp_dir().join(format!("llm-tutor-parent-pipe-test-{}", std::process::id()));
    let data_dir_arg = data_dir.to_string_lossy().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tutor-web"))
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--data-dir",
            &data_dir_arg,
            "--exit-on-stdin-close",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tutor-web should start for the parent pipe lifecycle test");

    thread::sleep(Duration::from_millis(500));
    if let Some(status) = child
        .try_wait()
        .expect("tutor-web status should be readable")
    {
        let error = child
            .stderr
            .take()
            .map(|mut stderr| {
                let mut message = String::new();
                stderr
                    .read_to_string(&mut message)
                    .expect("tutor-web stderr should be readable");
                message
            })
            .unwrap_or_default();
        panic!("tutor-web exited before its parent pipe closed ({status}): {error}");
    }

    drop(child.stdin.take());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child
            .try_wait()
            .expect("tutor-web status should be readable")
        {
            assert!(status.success(), "tutor-web exited with {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("tutor-web did not exit after its parent pipe closed");
        }
        thread::sleep(Duration::from_millis(50));
    }

    let _ = std::fs::remove_dir_all(data_dir);
}
