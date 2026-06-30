# Desktop Sidecars

Tauri v2 expects sidecar binaries declared in `bundle.externalBin` to use a
target-triple suffix.

For the Windows v0.1 desktop release, build `tutor-web` and copy it here as:

```text
tutor-web-x86_64-pc-windows-msvc.exe
```

The release build script automates this:

```powershell
.\scripts\build-desktop.ps1
```

`tauri.conf.json` intentionally does not declare the sidecar directly because
Tauri validates `externalBin` during normal `cargo check`. Release packaging
should merge `tauri.release.conf.json` after the sidecar binary has been copied
into this directory.
