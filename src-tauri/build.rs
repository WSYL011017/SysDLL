// Embed a Windows application manifest that requests `requireAdministrator`
// when the GUI runs as a *release* build, so a normal double-click triggers
// one UAC prompt and the process boots with an elevated token.
//
// Why dev keeps the default (asInvoker) manifest:
//   `tauri dev` is run from the user's normal shell; if we required admin
//   for every `cargo run`, the IDE terminal itself would have to be
//   elevated first. Production builds go through `npm run tauri build`
//   which uses release profile, so the manifest embeds there.
//
// See `app.manifest` for the trustInfo / Common-Controls blocks.

fn main() {
    // `tauri_build::is_dev()` flips true during `cargo build` from
    // `cargo` invoked by `tauri dev`, false during `tauri build`.
    // We can therefore install the elevated manifest on every non-dev
    // invocation without breaking the dev workflow.
    let mut windows = tauri_build::WindowsAttributes::new();
    if !tauri_build::is_dev() {
        windows = windows.app_manifest(include_str!("app.manifest"));
    }
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run tauri build script");
}
