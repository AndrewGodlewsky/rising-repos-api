//! The list an isolate holds between requests, and when it asks the site for the list again (rising-repos-site#175).

use crate::list::{List, ListError};

/// How long a list is used before the site is asked for it again: a new board day reaches the search within this.
pub const REFRESH_MS: f64 = 10.0 * 60.0 * 1000.0;

#[derive(Default)]
pub struct Cache {
    list: Option<List>,
    /// When the site was last asked for the list, in milliseconds since 1970, whether or not the answer was used.
    tried_at: Option<f64>,
}

impl Cache {
    pub fn list(&self) -> Option<&List> {
        self.list.as_ref()
    }

    /// Whether to ask the site for the list: when none is held, or ten minutes after it was last asked.
    pub fn needs_refresh(&self, now_ms: f64) -> bool {
        match (&self.list, self.tried_at) {
            (Some(_), Some(tried)) => now_ms - tried >= REFRESH_MS,
            _ => true,
        }
    }

    /// Take the site's `text` as the list. A text that cannot be read leaves the list held as it was.
    pub fn accept(&mut self, text: String, now_ms: f64) -> Result<(), ListError> {
        self.tried_at = Some(now_ms);
        self.list = Some(List::parse(text)?);
        Ok(())
    }

    /// The site could not be reached: keep the list held, and ask again in ten minutes.
    pub fn failed(&mut self, now_ms: f64) {
        self.tried_at = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/repos.tsv");

    #[test]
    fn refresh_after_ten_minutes() {
        let mut cache = Cache::default();
        assert!(cache.needs_refresh(0.0));
        cache.accept(FIXTURE.to_string(), 1_000.0).unwrap();
        assert!(!cache.needs_refresh(1_000.0 + REFRESH_MS - 1.0));
        assert!(cache.needs_refresh(1_000.0 + REFRESH_MS));
    }

    #[test]
    fn refresh_keeps_the_old_list_on_failure() {
        let mut cache = Cache::default();
        cache.accept(FIXTURE.to_string(), 0.0).unwrap();
        let later = REFRESH_MS + 5.0;
        assert_eq!(cache.accept("# rising-repos list, format 2\n".to_string(), later).unwrap_err().line, 1);
        assert_eq!(cache.list().map(List::len), Some(5));
        assert!(!cache.needs_refresh(later + 1.0), "a failed refresh is not tried again at once");
        assert!(cache.needs_refresh(later + REFRESH_MS));
        cache.failed(later + REFRESH_MS);
        assert_eq!(cache.list().map(List::len), Some(5));
        assert!(!cache.needs_refresh(later + REFRESH_MS + 1.0));
    }

    #[test]
    fn with_no_list_held_every_request_tries_again() {
        let mut cache = Cache::default();
        assert!(cache.accept(String::new(), 0.0).is_err());
        assert!(cache.list().is_none());
        assert!(cache.needs_refresh(1.0));
        cache.failed(2.0);
        assert!(cache.needs_refresh(3.0));
    }
}
