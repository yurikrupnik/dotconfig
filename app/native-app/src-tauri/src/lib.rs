use api_core::{greet as core_greet, health as core_health, GreetResponse, HealthResponse};

// Tauri command that delegates to shared API core
#[tauri::command]
fn greet(name: &str) -> GreetResponse {
    GreetResponse {
        message: core_greet(name),
    }
}

#[tauri::command]
fn health() -> HealthResponse {
    core_health()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, health])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
