//! The list an isolate holds between requests, and when it asks the site for the list again (rising-repos-site#175).
//!
//! An isolate can run several requests at once, each taking turns at an await, so a request that finds the list due
//! marks a fetch as begun before it waits on the site. The others then answer from the list held, or, when there is
//! none yet, wait for that one fetch rather than start their own (the final review of rising-repos-api#1).

use crate::list::{List, ListError};

/// How long a list is used before the site is asked for it again: a new board day reaches the search within this.
pub const REFRESH_MS: f64 = 10.0 * 60.0 * 1000.0;

/// How long a fetch may be under way before it is taken to have been lost, and another is begun.
pub const GIVE_UP_MS: f64 = 30.0 * 1000.0;

/// What a request does about the list, which `Cache::plan` says.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Answer from the list held.
    Use,
    /// Answer from the list held, and fetch the list again behind the answer.
    RefreshBehind,
    /// No list is held and none is being fetched: fetch it, and answer once it is read.
    Fetch,
    /// No list is held and another request is fetching it: wait for that.
    Wait,
}

#[derive(Default)]
pub struct Cache {
    list: Option<List>,
    /// When the site last answered or failed, in milliseconds since 1970, whether or not the answer was used.
    tried_at: Option<f64>,
    /// When the fetch under way began, if one is.
    fetching: Option<f64>,
}

impl Cache {
    pub fn list(&self) -> Option<&List> {
        self.list.as_ref()
    }

    /// What a request at `now_ms` does about the list. When it says to fetch, the fetch is marked as begun.
    pub fn plan(&mut self, now_ms: f64) -> Plan {
        let under_way = self.fetching.is_some_and(|began| now_ms - began < GIVE_UP_MS);
        let fresh = self.tried_at.is_some_and(|tried| now_ms - tried < REFRESH_MS);
        match (self.list.is_some(), under_way) {
            (true, true) => Plan::Use,
            (true, false) if fresh => Plan::Use,
            (false, true) => Plan::Wait,
            (held, false) => {
                self.fetching = Some(now_ms);
                if held {
                    Plan::RefreshBehind
                } else {
                    Plan::Fetch
                }
            }
        }
    }

    /// Take the site's `text` as the list. A text that cannot be read leaves the list held as it was.
    pub fn accept(&mut self, text: String, now_ms: f64) -> Result<(), ListError> {
        self.failed(now_ms);
        self.list = Some(List::parse(text)?);
        Ok(())
    }

    /// The fetch ended with nothing to use: keep the list held, and ask again in ten minutes, or, with none held, at
    /// the next request.
    pub fn failed(&mut self, now_ms: f64) {
        self.fetching = None;
        self.tried_at = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/repos.tsv");

    fn held() -> Cache {
        let mut cache = Cache::default();
        assert_eq!(cache.plan(0.0), Plan::Fetch);
        cache.accept(FIXTURE.to_string(), 0.0).unwrap();
        cache
    }

    #[test]
    fn refresh_after_ten_minutes() {
        let mut cache = held();
        assert_eq!(cache.plan(REFRESH_MS - 1.0), Plan::Use);
        assert_eq!(cache.plan(REFRESH_MS), Plan::RefreshBehind);
    }

    #[test]
    fn refresh_keeps_the_old_list_on_failure() {
        let mut cache = held();
        let later = REFRESH_MS + 5.0;
        assert_eq!(cache.plan(later), Plan::RefreshBehind);
        assert_eq!(cache.accept("# rising-repos list, format 2\n".to_string(), later).unwrap_err().line, 1);
        assert_eq!(cache.list().map(|list| list.repos().count()), Some(5));
        assert_eq!(cache.plan(later + 1.0), Plan::Use, "a failed refresh is not tried again at once");
        assert_eq!(cache.plan(later + REFRESH_MS), Plan::RefreshBehind);
        cache.failed(later + REFRESH_MS);
        assert_eq!(cache.list().map(|list| list.repos().count()), Some(5));
        assert_eq!(cache.plan(later + REFRESH_MS + 1.0), Plan::Use);
    }

    #[test]
    fn with_no_list_held_every_request_tries_again() {
        let mut cache = Cache::default();
        assert_eq!(cache.plan(0.0), Plan::Fetch);
        assert!(cache.accept(String::new(), 0.0).is_err());
        assert!(cache.list().is_none());
        assert_eq!(cache.plan(1.0), Plan::Fetch);
        cache.failed(2.0);
        assert_eq!(cache.plan(3.0), Plan::Fetch);
    }

    #[test]
    fn requests_during_a_fetch_do_not_fetch_again() {
        let mut cache = Cache::default();
        assert_eq!(cache.plan(0.0), Plan::Fetch);
        assert_eq!(cache.plan(10.0), Plan::Wait);
        assert_eq!(cache.plan(20.0), Plan::Wait);
        cache.accept(FIXTURE.to_string(), 380.0).unwrap();
        assert_eq!(cache.plan(381.0), Plan::Use);

        let due = 380.0 + REFRESH_MS;
        assert_eq!(cache.plan(due), Plan::RefreshBehind);
        assert_eq!(cache.plan(due + 10.0), Plan::Use);
        assert_eq!(cache.plan(due + 20.0), Plan::Use);
    }

    #[test]
    fn a_fetch_that_never_ends_is_given_up() {
        let mut cache = Cache::default();
        assert_eq!(cache.plan(0.0), Plan::Fetch);
        assert_eq!(cache.plan(GIVE_UP_MS - 1.0), Plan::Wait);
        assert_eq!(cache.plan(GIVE_UP_MS), Plan::Fetch);

        let mut cache = held();
        assert_eq!(cache.plan(REFRESH_MS), Plan::RefreshBehind);
        assert_eq!(cache.plan(REFRESH_MS + GIVE_UP_MS - 1.0), Plan::Use);
        assert_eq!(cache.plan(REFRESH_MS + GIVE_UP_MS), Plan::RefreshBehind);
    }
}
