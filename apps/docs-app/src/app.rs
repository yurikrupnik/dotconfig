use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Meta, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use crate::components::{DocPage, HomePage, NotFound, Sidebar};

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Meta charset="utf-8"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1"/>
        <Stylesheet id="leptos" href="/pkg/docs-app.css"/>
        <Title text="Platform Operator Docs"/>

        <Router>
            <div class="app-container">
                <Sidebar/>
                <main class="content">
                    <Routes fallback=|| view! { <NotFound/> }>
                        <Route path=path!("/") view=HomePage/>
                        <Route path=path!("/docs/*path") view=DocPage/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
