use leptos::prelude::*;

use crate::docs::{search_docs, SearchResult};

#[component]
pub fn SearchBox() -> impl IntoView {
    let (query, set_query) = signal(String::new());
    let (show_results, set_show_results) = signal(false);

    let search_results = Resource::new(
        move || query.get(),
        |q| async move {
            if q.len() < 2 {
                return Ok(vec![]);
            }
            search_docs(&q).await
        },
    );

    let on_input = move |ev| {
        let value = event_target_value(&ev);
        set_query.set(value);
        set_show_results.set(true);
    };

    let on_blur = move |_| {
        // Delay hiding to allow click on results
        set_timeout(
            move || set_show_results.set(false),
            std::time::Duration::from_millis(200),
        );
    };

    view! {
        <div class="search-container">
            <input
                type="search"
                placeholder="Search documentation..."
                class="search-input"
                prop:value=move || query.get()
                on:input=on_input
                on:focus=move |_| set_show_results.set(true)
                on:blur=on_blur
            />

            <Show when=move || show_results.get() && query.get().len() >= 2>
                <div class="search-results">
                    <Suspense fallback=move || view! { <p class="searching">"Searching..."</p> }>
                        {move || {
                            search_results.get().map(|results| {
                                match results {
                                    Ok(results) if results.is_empty() => {
                                        view! { <p class="no-results">"No results found"</p> }.into_any()
                                    }
                                    Ok(results) => {
                                        view! { <SearchResultsList results=results/> }.into_any()
                                    }
                                    Err(e) => {
                                        view! { <p class="search-error">{e.to_string()}</p> }.into_any()
                                    }
                                }
                            })
                        }}
                    </Suspense>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn SearchResultsList(results: Vec<SearchResult>) -> impl IntoView {
    view! {
        <ul class="results-list">
            {results
                .into_iter()
                .map(|result| {
                    view! {
                        <li>
                            <a href=format!("/docs/{}", result.slug)>
                                <strong>{result.title}</strong>
                                {result.excerpt.map(|e| view! { <p class="excerpt">{e}</p> })}
                            </a>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}
