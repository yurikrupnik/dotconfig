import { createSignal, Show } from "solid-js";
import logo from "./assets/logo.svg";
import "./App.css";

// Types
interface GreetResponse {
  message: string;
}

// Check if running inside Tauri
const isTauri = (): boolean => {
  return typeof window !== "undefined" && "__TAURI__" in window;
};

// Get API base URL
const getApiUrl = (): string => {
  // In production, this could be configured via environment
  return import.meta.env.VITE_API_URL || "http://localhost:3000";
};

// Call the greet API - works in both Tauri and browser
async function callGreet(name: string): Promise<string> {
  if (isTauri()) {
    // Use Tauri invoke
    const { invoke } = await import("@tauri-apps/api/core");
    const response = await invoke<GreetResponse>("greet", { name });
    return response.message;
  } else {
    // Use HTTP API
    const response = await fetch(`${getApiUrl()}/api/greet`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ name }),
    });

    if (!response.ok) {
      throw new Error(`HTTP error: ${response.status}`);
    }

    const data: GreetResponse = await response.json();
    return data.message;
  }
}

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [errorMsg, setErrorMsg] = createSignal("");
  const [name, setName] = createSignal("");
  const [isLoading, setIsLoading] = createSignal(false);

  const mode = isTauri() ? "Tauri" : "Browser";

  async function greet() {
    const currentName = name();
    if (!currentName.trim()) {
      return;
    }

    setIsLoading(true);
    setErrorMsg("");

    try {
      const message = await callGreet(currentName);
      setGreetMsg(message);
    } catch (error) {
      setErrorMsg(`Error: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <main class="container">
      <h1>Welcome to Tauri + Solid</h1>

      <div class="row">
        <a href="https://vite.dev" target="_blank">
          <img src="/vite.svg" class="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" class="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://solidjs.com" target="_blank">
          <img src={logo} class="logo solid" alt="Solid logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and Solid logos to learn more.</p>
      <p class="mode-indicator">Running in: {mode} mode</p>

      <form
        class="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
          disabled={isLoading()}
        />
        <button type="submit" disabled={isLoading()}>
          {isLoading() ? "Loading..." : "Greet"}
        </button>
      </form>

      <p class="greet-message">{greetMsg()}</p>
      <Show when={errorMsg()}>
        <p class="error-message" style={{ color: "red" }}>{errorMsg()}</p>
      </Show>
    </main>
  );
}

export default App;
