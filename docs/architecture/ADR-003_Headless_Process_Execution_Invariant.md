# ADR-003: Headless Process Execution Invariant on Windows

## Status
Accepted

## Date
2026-11-19

## Context
On Windows desktop environments, executing background utilities, maintenance tasks, or closing persistent background daemons via context menu or tray occasionally triggered a brief flashing command prompt / console window.
This console flash broke user immersion and gave the application an unpolished, fragile feel.

## Decision
1. **Zero-Console Invariant (`CREATE_NO_WINDOW`)**:
   - Any background process, installer maintenance action, uninstaller invocation, or CLI probe launched by the application must specify `creation_flags(0x08000000)` (`CREATE_NO_WINDOW`) on Windows.
   - Standard input, output, and error streams must be redirected or safely consumed via non-blocking pipes.
2. **Native Win32 GUI Subsystem**:
   - `ide-fast` and `IDE1.0.exe` are compiled with `#![windows_subsystem = "windows"]`, ensuring no parent console window is ever attached at binary startup.
3. **Headless Maintenance Mode**:
   - When the installer is invoked in maintenance/repair mode (`--uninstall` or `--maintenance`) on an already-installed instance, it starts as a native GUI window without spawning visible command shells.
   - File copy/removal use `std::fs`. PATH, shortcut and process-stop steps run through `headless_cmd("powershell")` (`CREATE_NO_WINDOW`, stdin `null`) executing **static** script constants; every runtime value (paths, PID, new PATH) is passed via `IDE10_*` environment variables, never interpolated into script text. Registry keys are written/deleted via `reg.exe` under the same headless wrapper. There is no `cmd.exe /c` and no `unsafe` Win32 FFI (`#![forbid(unsafe_code)]`).
   - User PATH is edited read-modify-write in pure Rust (`add_dir_to_path_str` / `remove_dir_from_path_str`); if the current value cannot be read, nothing is written, so a user PATH is never clobbered.
4. **Non-blocking Deployment**:
   - Real (non dry-run) install/repair/uninstall runs on a dedicated `ide-deploy` thread over an immutable `DeployPlan`; the UI thread only polls the result channel. Partial failures (permissions, PATH not updated) are surfaced through `status_msg` as a visible warning card instead of being swallowed.
## Consequences
- **Positive**: Complete elimination of black terminal flashes during startup, maintenance, repair, or tray shutdown.
- **Verification**: Verified via `test_installer_headless_startup_and_subsystem_invariants`, `test_path_env_add_remove_and_hex_decoding` and `test_dry_run_finishes_without_background_thread_and_real_job_is_isolated`.
