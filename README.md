# rising-repos-api

The query service of [risingrepos.com](https://risingrepos.com): a Cloudflare Worker, written in Rust, that answers a
visiting agent's search across every repository that has been on a Rising Repos board. A visiting agent is a program,
usually driven by a language model, that reads the site on a person's behalf.

**Not live yet.** It is being built from [the plan](docs/plans/2026-11-27-query-service.md) for #1, and goes live
once its deploy has the owner's Cloudflare token. Its design is settled in the planning of
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

## Building and testing

Rust is pinned in `rust-toolchain.toml`, with the `wasm32-unknown-unknown` target, and rustup installs that version
the first time a command runs here. Every build and test reads `Cargo.lock` as it is, with `--locked`.

```sh
cargo test --locked                                       # the tests: they run on this machine, not in a Worker
cargo install worker-build --version 0.9.0 --locked       # once: the tool that builds the Worker
worker-build --release                                    # the Worker, into build/
```

On Windows, installing `worker-build` needs Rust's MSVC toolchain and Visual Studio's build tools, since the GNU
toolchain could not build it (rising-repos-site#174). Once it is installed, `worker-build` builds the Worker with
either.

**The versions.** A version moves only once it is free of known vulnerabilities and at least 14 days old (the site's
STANDARDS.md, C8). Two crates the lock holds were 6 days old when it was made on 27 November 2026: `url` 2.5.9 and
`form_urlencoded` 1.2.3, which it needs. `url` 2.5.9 fixes RUSTSEC-2026-0592, a stack overflow in `Url::origin()`,
and 2.5.8 before it is affected, so the fix was kept: the two turn 14 days old on 4 December 2026. `chrono`, which the
`worker` crate needs, carries RUSTSEC-2026-0543, a notice that it is no longer maintained, which every version of it
does; this Worker's own code does not use it.
