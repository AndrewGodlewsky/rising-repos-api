//! A search across every repository in the list, and the lookup of one by its name (rising-repos-site#173).

use crate::list::{lowercase, List, Repo};
use crate::route::SearchQuery;

/// Every repository whose name or description holds each of the query's words, in the list's order: newest last
/// board day first, then the better rank that day. A query with no words matches nothing, so that no answer is the
/// whole list.
pub fn search<'a>(list: &'a List, query: &SearchQuery) -> Vec<Repo<'a>> {
    if query.words.is_empty() {
        return Vec::new();
    }
    let since = query.since.and_then(|count| list.oldest_of_newest_days(count));
    // The longest word first: it is the likeliest to be missing, which ends a repository's check soonest.
    let mut words: Vec<&str> = query.words.iter().map(String::as_str).collect();
    words.sort_by_key(|word| std::cmp::Reverse(word.len()));
    list.repos()
        .take_while(|repo| since.is_none_or(|day| repo.last_board_day >= day))
        .filter(|repo| query.lang.as_deref().is_none_or(|lang| same_language(lang, repo.language)))
        .filter(|repo| words.iter().all(|word| repo.haystack.contains(word)))
        .collect()
}

/// The repository named `owner/name`, whatever the case of the letters asked for.
pub fn find<'a>(list: &'a List, owner: &str, name: &str) -> Option<Repo<'a>> {
    list.repos().find(|repo| {
        repo.full_name
            .split_once('/')
            .is_some_and(|(its_owner, its_name)| its_owner.eq_ignore_ascii_case(owner) && its_name.eq_ignore_ascii_case(name))
    })
}

fn same_language(asked: &str, language: &str) -> bool {
    asked.eq_ignore_ascii_case(language) || (!asked.is_ascii() && lowercase(asked) == lowercase(language))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> List {
        List::parse(include_str!("../tests/fixtures/repos.tsv").to_string()).unwrap()
    }

    fn query(words: &[&str], lang: Option<&str>, since: Option<usize>) -> SearchQuery {
        SearchQuery {
            words: words.iter().map(|word| word.to_string()).collect(),
            lang: lang.map(str::to_string),
            since,
            page: 1,
            json: false,
        }
    }

    fn names(list: &List, query: &SearchQuery) -> Vec<String> {
        search(list, query).into_iter().map(|repo| repo.full_name.to_string()).collect()
    }

    #[test]
    fn every_word_must_match_case_blind() {
        let list = list();
        assert_eq!(names(&list, &query(&["pdf"], None, None)), ["pdf-labs/PDF-Kit", "tinyorg/pdfquote"]);
        assert_eq!(names(&list, &query(&["pdf", "reader"], None, None)), ["tinyorg/pdfquote"]);
        assert!(names(&list, &query(&["pdf", "flask"], None, None)).is_empty());
    }

    #[test]
    fn lang_filters_case_blind() {
        let list = list();
        assert_eq!(names(&list, &query(&["a"], Some("python"), None)), ["pallets/flask"]);
        assert_eq!(names(&list, &query(&["a"], Some("RUST"), None)), ["newowner/renamed-tool"]);
        assert!(names(&list, &query(&["a"], Some("Py"), None)).is_empty());
    }

    #[test]
    fn since_keeps_the_newest_n_board_days() {
        let list = list();
        let newest = ["pdf-labs/PDF-Kit", "tinyorg/pdfquote", "pallets/flask"];
        assert_eq!(names(&list, &query(&["a"], None, Some(1))), newest);
        assert_eq!(names(&list, &query(&["a"], None, Some(2))).len(), 4);
        assert_eq!(names(&list, &query(&["a"], None, Some(3))).len(), 5);
        assert_eq!(names(&list, &query(&["a"], None, Some(100))).len(), 5);
    }

    #[test]
    fn order_is_newest_board_day_then_rank() {
        assert_eq!(
            names(&list(), &query(&["a"], None, None)),
            ["pdf-labs/PDF-Kit", "tinyorg/pdfquote", "pallets/flask", "newowner/renamed-tool", "acme/old-days"]
        );
    }

    #[test]
    fn empty_query_matches_nothing() {
        let list = list();
        assert!(search(&list, &query(&[], None, None)).is_empty());
        assert!(search(&list, &query(&[], Some("Python"), Some(7))).is_empty());
    }

    #[test]
    fn repo_lookup_is_case_blind() {
        let list = list();
        assert_eq!(find(&list, "PALLETS", "Flask").map(|repo| repo.full_name), Some("pallets/flask"));
        assert_eq!(find(&list, "pdf-labs", "pdf-kit").map(|repo| repo.full_name), Some("pdf-labs/PDF-Kit"));
        assert_eq!(find(&list, "pallets", "nothing"), None);
        assert_eq!(find(&list, "pallets/flask", ""), None);
    }
}
