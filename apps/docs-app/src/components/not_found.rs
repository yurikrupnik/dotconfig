use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h1>"404"</h1>
            <h2>"Page Not Found"</h2>
            <p>"The page you're looking for doesn't exist or has been moved."</p>
            <A href="/" class="back-home">"Back to Home"</A>
        </div>
    }
}
