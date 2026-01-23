use leptos::task::spawn_local;
use leptos::{ev::SubmitEvent, prelude::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// Tauri invoke binding (only used when running in Tauri)
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize)]
struct GreetArgs {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct GreetResponse {
    message: String,
}

/// Check if running inside Tauri
fn is_tauri() -> bool {
    if let Some(window) = web_sys::window() {
        js_sys::Reflect::get(&window, &"__TAURI__".into())
            .map(|v| !v.is_undefined())
            .unwrap_or(false)
    } else {
        false
    }
}

/// Get the API base URL from environment or default
fn get_api_url() -> String {
    // In production, this could be configured via environment
    // For local development, use localhost:3000
    "http://localhost:3000".to_string()
}

/// Call the greet API - works in both Tauri and browser
async fn call_greet(name: &str) -> Result<String, String> {
    if is_tauri() {
        // Use Tauri invoke
        let args = serde_wasm_bindgen::to_value(&GreetArgs {
            name: name.to_string(),
        })
        .map_err(|e| e.to_string())?;

        let result = invoke("greet", args).await;
        let response: GreetResponse =
            serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())?;
        Ok(response.message)
    } else {
        // Use HTTP API
        let url = format!("{}/api/greet", get_api_url());
        let request = GreetArgs {
            name: name.to_string(),
        };

        let response = gloo_net::http::Request::post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.ok() {
            let data: GreetResponse = response.json().await.map_err(|e| e.to_string())?;
            Ok(data.message)
        } else {
            Err(format!("HTTP error: {}", response.status()))
        }
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (greet_msg, set_greet_msg) = signal(String::new());
    let (error_msg, set_error_msg) = signal(String::new());
    let (is_loading, set_is_loading) = signal(false);

    // Show mode indicator
    let mode = if is_tauri() { "Tauri" } else { "Browser" };

    let update_name = move |ev| {
        let v = event_target_value(&ev);
        set_name.set(v);
    };

    let greet = move |ev: SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let name_value = name.get_untracked();
            if name_value.is_empty() {
                return;
            }

            set_is_loading.set(true);
            set_error_msg.set(String::new());

            match call_greet(&name_value).await {
                Ok(msg) => set_greet_msg.set(msg),
                Err(e) => set_error_msg.set(format!("Error: {}", e)),
            }

            set_is_loading.set(false);
        });
    };

    view! {
        <main class="container">
            <h1>"Welcome to Tauri + Leptos"</h1>

            <div class="row">
                <a href="https://tauri.app" target="_blank">
                    <img src="public/tauri.svg" class="logo tauri" alt="Tauri logo"/>
                </a>
                <a href="https://docs.rs/leptos/" target="_blank">
                    <img src="public/leptos.svg" class="logo leptos" alt="Leptos logo"/>
                </a>
            </div>
            <p>"Click on the Tauri and Leptos logos to learn more."</p>
            <p class="mode-indicator">"Running in: " {mode} " mode"</p>

            <form class="row" on:submit=greet>
                <input
                    id="greet-input"
                    placeholder="Enter a name..."
                    on:input=update_name
                    disabled=move || is_loading.get()
                />
                <button type="submit" disabled=move || is_loading.get()>
                    {move || if is_loading.get() { "Loading..." } else { "Greet" }}
                </button>
            </form>

            <p class="greet-message">{ move || greet_msg.get() }</p>
            <p class="error-message" style="color: red;">{ move || error_msg.get() }</p>
        </main>
    }
}
