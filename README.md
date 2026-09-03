# NotePad Pro

NotePad Pro 1.0.2-scintilla is a native Rust note editor. The workspace has a headless `notepad-pro-core` crate for document metadata, list behaviour, encoding and line-ending conversion, atomic persistence, SQLite notes, settings/session storage, searching, and highlight extraction. The `notepad-pro` crate provides a frameless `winit` shell, `softbuffer` presentation, `tiny-skia` chrome, a small font rasterizer, and a direct Scintilla FFI bridge.

## Build

Rust 1.78 or newer is required. On Linux:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p notepad-pro
./packaging/build-linux.sh
```

The Linux script creates a `.deb` and a portable `.tar.gz` under `artifacts/`. The application stores settings, session state, recent files, and its WAL-enabled SQLite note index under the platform data directory; `NOTEPAD_PRO_DATA_DIR` overrides it for tests and portable deployments.

On Windows, the target-specific `scintilla-sys` 5.5.7 dependency builds the static Scintilla core with the `win32` feature. Install the MSVC Rust target and WiX Toolset 4, then run from PowerShell:

```powershell
$env:NOTEPAD_TARGET = 'x86_64-pc-windows-msvc'
./packaging/build-windows.ps1
```

That produces `artifacts/NotePad-Pro-1.0.2.msi`. No browser engine or web runtime is included. The shell uses a software editor surface on backends that cannot host a Scintilla child window; Windows uses the target-specific static Scintilla child host. On Unix distributors may provide a compatible Scintilla source tree or static archive through `NOTEPAD_SCINTILLA_DIR` or `NOTEPAD_SCINTILLA_STATIC` for the optional build bridge.

## Verification

The GitHub workflow runs the native Linux test, clippy, and release-build checks on stable Rust; the source remains compatible with the pinned Rust 1.78 baseline where dependency versions permit it. Core persistence and editor behavior are also covered by integration tests.

## CLI

```text
notepad-pro [FILE ...]
```

Files are decoded to an internal UTF-8 buffer, line endings are normalized to LF while editing, and saves use the original encoding/line-ending metadata with a temporary-file replacement.
