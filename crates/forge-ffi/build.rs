//! Generates the Tauri context — the parsed configuration, the bundled front
//! end and the capability set — that `tauri::generate_context!` expands into.

fn main() {
    tauri_build::build()
}
