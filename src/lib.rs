//! The query service of risingrepos.com. The modules hold the logic and its tests; this file only joins them to
//! Cloudflare's runtime: it reads the request, fetches the site's list when the isolate needs it, and sends the answer.

mod cache;
mod list;
mod render;
mod route;
mod search;

use std::cell::RefCell;
use std::time::Duration;

use worker::{console_error, event, Context, Date, Delay, Env, Fetch, Request, Response, Result};

use crate::cache::{Cache, Plan};
use crate::render::MARKDOWN;
use crate::route::Route;

/// The site's list of every repository ever on a board (rising-repos-site#177).
const LIST_URL: &str = "https://risingrepos.com/repos.tsv";

/// The search's own address, which a page's next address is made from.
const SEARCH_URL: &str = "https://risingrepos.com/api/search";

/// How long a request that waits for another's fetch sleeps between looks at the cache.
const WAIT_STEP: Duration = Duration::from_millis(25);

thread_local! {
    /// The list this isolate holds between requests. The isolate has one thread, but it can run several requests at
    /// once, taking turns at each await, so no borrow of the cache is held across an await, and `Cache::plan` marks
    /// a fetch as begun before its request waits on the site.
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

#[event(fetch)]
async fn fetch(request: Request, _env: Env, context: Context) -> Result<Response> {
    let url = request.url()?;
    let query: Vec<(String, String)> = url.query_pairs().map(|(key, value)| (key.into_owned(), value.into_owned())).collect();
    let route = route::route(url.path(), &query);
    if route == Route::NotFound {
        return answer(render::not_found(), MARKDOWN, 404);
    }
    loop {
        let now = now_ms();
        match CACHE.with(|cache| cache.borrow_mut().plan(now)) {
            Plan::Use => break,
            // The answer goes out from the list held, and the fetch and the reading of the new list run after it.
            Plan::RefreshBehind => {
                context.wait_until(refresh(now));
                break;
            }
            Plan::Fetch => {
                refresh(now).await;
                break;
            }
            // Another request is fetching the first list. Sleeping costs no CPU; once its fetch ends, the plan says
            // to use the list, or, if it failed, to fetch again.
            Plan::Wait => Delay::from(WAIT_STEP).await,
        }
    }
    CACHE.with(|cache| {
        let cache = cache.borrow();
        let Some(list) = cache.list() else {
            return answer(render::not_loaded(), MARKDOWN, 503);
        };
        match &route {
            Route::Search(asked) => {
                let (body, kind) = render::search_answer(&search::search(list, asked), asked, SEARCH_URL);
                answer(body, kind, 200)
            }
            Route::Repo { owner, name, json } => match search::find(list, owner, name) {
                Some(repo) => {
                    let (body, kind) = render::repo_answer(&repo, *json);
                    answer(body, kind, 200)
                }
                None => answer(render::repo_not_found(owner, name), MARKDOWN, 404),
            },
            Route::NotFound => answer(render::not_found(), MARKDOWN, 404),
        }
    })
}

fn now_ms() -> f64 {
    Date::now().as_millis() as f64
}

/// Ask the site for the list, and keep it if it can be read. A failure keeps the list held, and is logged.
async fn refresh(began: f64) {
    let fetched = fetch_list().await;
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        match fetched {
            Ok(text) => {
                if let Err(error) = cache.accept(text, began) {
                    console_error!("the list from {LIST_URL} was not used: {error}");
                }
            }
            Err(error) => {
                cache.failed(began);
                console_error!("the list could not be fetched from {LIST_URL}: {error}");
            }
        }
    });
}

/// The list's text. Its bytes are taken as they came and checked as UTF-8, which costs less CPU than having the
/// runtime turn the text into a string first.
async fn fetch_list() -> std::result::Result<String, String> {
    let mut response = Fetch::Url(LIST_URL.parse().map_err(|error| format!("{error}"))?)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status_code() != 200 {
        return Err(format!("it answered {}", response.status_code()));
    }
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn answer(body: String, kind: &str, status: u16) -> Result<Response> {
    let response = Response::ok(body)?.with_status(status);
    let headers = response.headers();
    headers.set("Content-Type", kind)?;
    headers.set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}
