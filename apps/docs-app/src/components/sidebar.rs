use leptos::prelude::*;
use leptos_router::components::A;

use crate::docs::DocTree;

#[component]
pub fn Sidebar() -> impl IntoView {
    let doc_tree = Resource::new(|| (), |_| async move { DocTree::load().await });

    view! {
        <aside class="sidebar">
            <div class="sidebar-header">
                <A href="/" class="logo">
                    <h2>"Platform Operator"</h2>
                </A>
            </div>

            <nav class="sidebar-nav">
                <Suspense fallback=move || view! { <p>"Loading docs..."</p> }>
                    {move || {
                        doc_tree.get().map(|tree| {
                            match tree {
                                Ok(tree) => view! { <DocTreeView tree=tree/> }.into_any(),
                                Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </nav>

            <div class="sidebar-footer">
                <a href="https://github.com/yurikrupnik/dotconfig" target="_blank" class="github-link">
                    "GitHub"
                </a>
            </div>
        </aside>
    }
}

#[component]
fn DocTreeView(tree: DocTree) -> impl IntoView {
    view! {
        <ul class="doc-tree">
            {tree
                .sections
                .into_iter()
                .map(|section| {
                    view! {
                        <li class="doc-section">
                            <span class="section-title">{section.title}</span>
                            <ul class="section-pages">
                                {section
                                    .pages
                                    .into_iter()
                                    .map(|page| {
                                        view! {
                                            <li>
                                                <A href=format!("/docs/{}", page.slug)>{page.title}</A>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}
