//! Which of the two addresses a request asks for, and what its query says.

/// The most characters of `q` that a search reads (rising-repos-site#173).
pub const QUERY_LIMIT: usize = 100;

/// The most words of `q` that a search reads. Each word is looked for in every repository, so many short words would
/// multiply a search's CPU against the Free plan's 10 ms (the final review of rising-repos-api#1).
pub const WORD_LIMIT: usize = 8;

#[derive(Debug, PartialEq)]
pub struct SearchQuery {
    /// The words of `q`, lowercased. A piece with no letter or digit in it is not a word.
    pub words: Vec<String>,
    pub lang: Option<String>,
    /// The newest this many board days, when it is a number above 0.
    pub since: Option<usize>,
    /// The page asked for, counted from 1.
    pub page: usize,
    pub json: bool,
}

#[derive(Debug, PartialEq)]
pub enum Route {
    Search(SearchQuery),
    Repo { owner: String, name: String, json: bool },
    NotFound,
}

/// The address a request names, read from its path and its query. Only `/api/search` and `/api/repo/<owner>/<name>`
/// exist; every other path is `NotFound`. A key given twice is read where it first appears.
pub fn route(path: &str, query: &[(String, String)]) -> Route {
    let value = |key: &str| query.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str());
    let json = value("format") == Some("json");
    if path == "/api/search" {
        return Route::Search(SearchQuery {
            words: words(value("q").unwrap_or("")),
            lang: value("lang").filter(|lang| !lang.is_empty()).map(str::to_string),
            since: value("since").and_then(above_0),
            page: value("page").and_then(above_0).unwrap_or(1),
            json,
        });
    }
    match path.strip_prefix("/api/repo/").map(|rest| rest.split('/').collect::<Vec<_>>()).as_deref() {
        Some([owner, name]) if !owner.is_empty() && !name.is_empty() => {
            Route::Repo { owner: owner.to_string(), name: name.to_string(), json }
        }
        _ => Route::NotFound,
    }
}

/// The words of a query cut to `QUERY_LIMIT` characters: its pieces between white space, lowercased, leaving out a
/// piece with no letter or digit, so that a query of spaces or punctuation has no word to match. A word is read once,
/// and no more than `WORD_LIMIT` of them.
fn words(q: &str) -> Vec<String> {
    let cut: String = q.chars().take(QUERY_LIMIT).collect();
    let mut words: Vec<String> = Vec::new();
    for word in cut.split_whitespace().filter(|piece| piece.chars().any(char::is_alphanumeric)).map(crate::list::lowercase) {
        if words.len() == WORD_LIMIT {
            break;
        }
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

fn above_0(text: &str) -> Option<usize> {
    text.parse::<usize>().ok().filter(|number| *number > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(given: &[(&str, &str)]) -> Vec<(String, String)> {
        given.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect()
    }

    fn search(given: &[(&str, &str)]) -> SearchQuery {
        match route("/api/search", &pairs(given)) {
            Route::Search(query) => query,
            other => panic!("expected a search, got {other:?}"),
        }
    }

    #[test]
    fn search_path_parses_words_lang_since_page_and_format() {
        let given = [("q", "PDF  Reader"), ("lang", "Rust"), ("since", "7"), ("page", "2"), ("format", "json")];
        assert_eq!(
            search(&given),
            SearchQuery {
                words: vec!["pdf".into(), "reader".into()],
                lang: Some("Rust".into()),
                since: Some(7),
                page: 2,
                json: true,
            }
        );
        assert_eq!(
            search(&[("q", "pdf")]),
            SearchQuery { words: vec!["pdf".into()], lang: None, since: None, page: 1, json: false }
        );
    }

    #[test]
    fn query_is_cut_to_100_characters() {
        let long = "é".repeat(150);
        assert_eq!(search(&[("q", &long)]).words, vec!["é".repeat(100)]);
    }

    #[test]
    fn a_query_of_spaces_or_punctuation_has_no_words() {
        assert!(search(&[("q", "   ")]).words.is_empty());
        assert!(search(&[("q", "!!! -- ...")]).words.is_empty());
        assert_eq!(search(&[("q", "c++ node.js !!")]).words, vec!["c++", "node.js"]);
    }

    #[test]
    fn a_word_given_twice_is_read_once() {
        assert_eq!(search(&[("q", "pdf PDF reader pdf")]).words, vec!["pdf", "reader"]);
    }

    #[test]
    fn a_query_reads_at_most_8_words() {
        let words = search(&[("q", "one two three four five six seven eight nine ten")]).words;
        assert_eq!(words, vec!["one", "two", "three", "four", "five", "six", "seven", "eight"]);
    }

    #[test]
    fn a_page_that_is_not_a_number_above_0_is_page_1() {
        for page in ["0", "-3", "two", ""] {
            assert_eq!(search(&[("q", "pdf"), ("page", page)]).page, 1, "page={page}");
        }
    }

    #[test]
    fn repo_path_gives_owner_and_name() {
        assert_eq!(
            route("/api/repo/pallets/flask", &[]),
            Route::Repo { owner: "pallets".into(), name: "flask".into(), json: false }
        );
        assert_eq!(
            route("/api/repo/pallets/flask", &pairs(&[("format", "json")])),
            Route::Repo { owner: "pallets".into(), name: "flask".into(), json: true }
        );
    }

    #[test]
    fn any_other_api_path_is_not_found() {
        for path in ["/api", "/api/", "/api/nothing", "/api/searchx", "/api/repo/pallets", "/api/repo/a/b/c", "/api/repo//flask"] {
            assert_eq!(route(path, &[]), Route::NotFound, "{path}");
        }
    }
}
