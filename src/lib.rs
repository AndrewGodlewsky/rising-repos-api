//! The query service of risingrepos.com. The modules hold the logic and its tests; this file only joins them to
//! Cloudflare's runtime.

mod list;
mod route;
mod search;

use worker::{event, Context, Env, Request, Response, Result};

#[event(fetch)]
async fn fetch(_request: Request, _env: Env, _context: Context) -> Result<Response> {
    Response::error("Not found", 404)
}
