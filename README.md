# rising-repos-api

The query service of [risingrepos.com](https://risingrepos.com): a Cloudflare Worker, written in Rust, that answers a
visiting agent's search across every repository that has been on a Rising Repos board. A visiting agent is a program,
usually driven by a language model, that reads the site on a person's behalf.

**Not built yet.** Its design is settled in the planning of
[AndrewGodlewsky/rising-repos-site#151](https://github.com/AndrewGodlewsky/rising-repos-site/issues/151):

- what it answers and at which addresses, `/api/search` and `/api/repo/<owner>/<name>` under risingrepos.com
  ([rising-repos-site#173](https://github.com/AndrewGodlewsky/rising-repos-site/issues/173));
- why Rust, after the same search was tried in Rust, JavaScript, Python, and Go
  ([rising-repos-site#174](https://github.com/AndrewGodlewsky/rising-repos-site/issues/174));
- why it is a repository of its own, how it gets its list, and how it is pinned, tested, and deployed
  ([rising-repos-site#175](https://github.com/AndrewGodlewsky/rising-repos-site/issues/175)).

## Why a repository of its own

The site's repository takes a daily data push that can change any file in it but a workflow, so its deploy uploads
static files alone. Code that runs on Cloudflare for every request lives here instead, where that push cannot reach
it. The Worker reads the list of repositories from a static file the site publishes, so a new board day reaches the
search with no deploy of this Worker.
