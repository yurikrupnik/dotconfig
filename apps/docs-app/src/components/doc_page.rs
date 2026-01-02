use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::docs::load_doc;

#[component]
pub fn DocPage() -> impl IntoView {
    let params = use_params_map();

    let doc_content = Resource::new(
        move || params.get().get("path").unwrap_or_default(),
        |path| async move { load_doc(&path).await },
    );

    view! {
        <article class="doc-page">
            <Suspense fallback=move || view! { <DocSkeleton/> }>
                {move || {
                    doc_content.get().map(|result| {
                        match result {
                            Ok(doc) => {
                                view! {
                                    <header class="doc-header">
                                        <h1>{doc.title}</h1>
                                        {doc.description.map(|desc| view! { <p class="doc-description">{desc}</p> })}
                                    </header>
                                    <div class="doc-content" inner_html=doc.html/>
                                    <footer class="doc-footer">
                                        {doc.last_modified.map(|date| {
                                            view! { <p class="last-modified">"Last modified: " {date}</p> }
                                        })}
                                    </footer>
                                }
                                    .into_any()
                            }
                            Err(e) => {
                                view! {
                                    <div class="doc-error">
                                        <h2>"Document Not Found"</h2>
                                        <p>{e.to_string()}</p>
                                    </div>
                                }
                                    .into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </article>
    }
}

#[component]
fn DocSkeleton() -> impl IntoView {
    view! {
        <div class="doc-skeleton">
            <div class="skeleton-title"></div>
            <div class="skeleton-line"></div>
            <div class="skeleton-line"></div>
            <div class="skeleton-line short"></div>
            <div class="skeleton-line"></div>
            <div class="skeleton-line"></div>
        </div>
    }
}
