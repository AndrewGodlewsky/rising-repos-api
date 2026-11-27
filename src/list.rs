//! The site's list of every repository ever on a board, `https://risingrepos.com/repos.tsv`, in format 1
//! (rising-repos-site#177).
//!
//! The list is about 2.2 MB of 11,000 lines, and it is read inside a request, which has 10 ms of CPU on the Free plan.
//! So it is kept as the one text it was fetched as, and each repository is a set of places in it, rather than ten
//! small strings of its own: copying them took longer than the limit.

use std::fmt;
use std::ops::Range;

/// The list's first line, in the one format this service reads.
pub const FORMAT_LINE: &str = "# rising-repos list, format 1";

/// The list's second line: the names of the fields of each line after it, in order.
pub const FIELDS: [&str; 7] = ["full_name", "language", "stars", "last_board_day", "last_rank", "history", "description"];

/// The list, parsed: every repository in it, newest last board day first, then the better rank that day.
pub struct List {
    text: String,
    /// Each repository's name and description, lowercased, one after another.
    lower: String,
    rows: Vec<Row>,
}

/// Where a repository's fields are in the list's text, and its figures.
struct Row {
    full_name: Range<usize>,
    language: Range<usize>,
    stars: u64,
    last_board_day: Range<usize>,
    /// The day as a number, 20261126, so that the order compares numbers.
    day: u32,
    last_rank: u32,
    history: Range<usize>,
    description: Range<usize>,
    haystack: Range<usize>,
}

/// One repository of the list, as the list's text says it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Repo<'a> {
    pub full_name: &'a str,
    /// Empty when GitHub names no language.
    pub language: &'a str,
    pub stars: u64,
    pub last_board_day: &'a str,
    pub last_rank: u32,
    /// Every board day it was on, with its rank there, oldest first.
    pub history: History<'a>,
    pub description: &'a str,
    /// The name and the description, lowercased, which a search's words are looked for in.
    pub haystack: &'a str,
}

/// A repository's board days and its rank on each, as the list writes them: `2026-11-25:182,2026-11-26:221`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct History<'a>(&'a str);

impl<'a> History<'a> {
    /// Each board day and the rank that day, oldest first. `parse` has read every one, so none is left out.
    pub fn days(self) -> impl Iterator<Item = (&'a str, u32)> {
        self.0.split(',').filter_map(day_and_rank)
    }
}

/// What is wrong with the list, and on which line, counted from 1.
#[derive(Debug, PartialEq)]
pub struct ListError {
    pub line: usize,
    pub what: String,
}

impl fmt::Display for ListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.what)
    }
}

impl List {
    /// The list in `text`. A list in another format, or with a line this service cannot read, is refused whole, so
    /// that the caller keeps the list it held. A line may end in a carriage return, which is not read as part of it.
    pub fn parse(text: String) -> Result<List, ListError> {
        let mut lines = Vec::new();
        let mut offset = 0;
        for line in text.strip_suffix('\n').unwrap_or(&text).split('\n') {
            let length = line.strip_suffix('\r').unwrap_or(line).len();
            lines.push(offset..offset + length);
            offset += line.len() + 1;
        }
        let refuse = |line: usize, what: String| ListError { line, what };
        match lines.first().map(|range| &text[range.clone()]) {
            Some(FORMAT_LINE) => {}
            Some("") | None => return Err(refuse(1, "the list is empty".into())),
            Some(other) => return Err(refuse(1, format!("the list says {other:?}, and this service reads {FORMAT_LINE:?}"))),
        }
        if lines.get(1).map(|range| &text[range.clone()]) != Some(FIELDS.join("\t").as_str()) {
            return Err(refuse(2, format!("the fields are not {}", FIELDS.join(", "))));
        }
        let mut lower = String::with_capacity(text.len() / 2);
        let mut rows = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate().skip(2) {
            rows.push(row(&text, line.clone(), &mut lower).map_err(|what| refuse(index + 1, what))?);
        }
        rows.sort_by(|a, b| b.day.cmp(&a.day).then(a.last_rank.cmp(&b.last_rank)));
        Ok(List { text, lower, rows })
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Every repository, newest last board day first, then the better rank that day.
    pub fn repos(&self) -> impl Iterator<Item = Repo<'_>> {
        self.rows.iter().map(|row| self.repo(row))
    }

    fn repo<'a>(&'a self, row: &Row) -> Repo<'a> {
        let text = |range: &Range<usize>| &self.text[range.clone()];
        Repo {
            full_name: text(&row.full_name),
            language: text(&row.language),
            stars: row.stars,
            last_board_day: text(&row.last_board_day),
            last_rank: row.last_rank,
            history: History(text(&row.history)),
            description: text(&row.description),
            haystack: &self.lower[row.haystack.clone()],
        }
    }
}

/// The row of the line at `line` in `text`, with its haystack written onto the end of `lower`.
fn row(text: &str, line: Range<usize>, lower: &mut String) -> Result<Row, String> {
    let mut fields: [Range<usize>; 7] = Default::default();
    let mut count = 0;
    let mut start = line.start;
    for piece in text[line].split('\t') {
        if let Some(field) = fields.get_mut(count) {
            *field = start..start + piece.len();
        }
        start += piece.len() + 1;
        count += 1;
    }
    if count != FIELDS.len() {
        return Err(format!("{count} fields, and a line has {}", FIELDS.len()));
    }
    let [full_name, language, stars, last_board_day, last_rank, history, description] = fields;
    let field = |range: &Range<usize>| &text[range.clone()];
    let number = |name: &str, range: &Range<usize>| {
        field(range).parse::<u64>().map_err(|_| format!("{name} is {:?}, not a number", field(range)))
    };
    let stars = number("stars", &stars)?;
    let rank = number("last_rank", &last_rank)?;
    let day = day_number(field(&last_board_day)).ok_or_else(|| format!("last_board_day is {:?}", field(&last_board_day)))?;
    let seen = field(&history);
    if seen.split(',').any(|one| day_and_rank(one).is_none()) {
        return Err(format!("history is {seen:?}, not board days and ranks"));
    }
    let haystack_start = lower.len();
    lowercase_onto(lower, field(&full_name));
    lower.push(' ');
    lowercase_onto(lower, field(&description));
    Ok(Row {
        full_name,
        language,
        stars,
        last_board_day,
        day,
        last_rank: u32::try_from(rank).map_err(|_| format!("last_rank is {rank}"))?,
        history,
        description,
        haystack: haystack_start..lower.len(),
    })
}

/// `text` lowercased onto the end of `out`, a character at a time, as a search's words are (`lowercase`).
pub fn lowercase_onto(out: &mut String, text: &str) {
    if text.is_ascii() {
        let start = out.len();
        out.push_str(text);
        out[start..].make_ascii_lowercase();
    } else {
        out.extend(text.chars().flat_map(char::to_lowercase));
    }
}

/// `text` lowercased a character at a time, as the list's haystacks are.
pub fn lowercase(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    lowercase_onto(&mut out, text);
    out
}

fn day_and_rank(seen: &str) -> Option<(&str, u32)> {
    let (day, rank) = seen.split_once(':')?;
    Some((day, rank.parse().ok()?)).filter(|_| day_number(day).is_some())
}

/// `2026-11-26` as 20261126, or nothing when the text is not a day written so.
fn day_number(day: &str) -> Option<u32> {
    let bytes = day.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let mut number = 0;
    for (_, &byte) in bytes.iter().enumerate().filter(|(place, _)| *place != 4 && *place != 7) {
        if !byte.is_ascii_digit() {
            return None;
        }
        number = number * 10 + u32::from(byte - b'0');
    }
    Some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/repos.tsv");

    fn parse(text: &str) -> Result<List, ListError> {
        List::parse(text.to_string())
    }

    fn named<'a>(list: &'a List, full_name: &str) -> Repo<'a> {
        list.repos().find(|repo| repo.full_name == full_name).expect(full_name)
    }

    fn line_of(error: Result<List, ListError>) -> usize {
        error.err().expect("the list was refused").line
    }

    #[test]
    fn parses_format_1() {
        let list = parse(FIXTURE).unwrap();
        assert_eq!(list.len(), 5);
        let flask = named(&list, "pallets/flask");
        assert_eq!(
            (flask.full_name, flask.language, flask.stars, flask.last_board_day, flask.last_rank, flask.description),
            ("pallets/flask", "Python", 75297, "2026-11-26", 221, "The Python micro framework for building web applications.")
        );
        assert_eq!(flask.history.days().collect::<Vec<_>>(), [("2026-08-03", 1), ("2026-08-04", 5), ("2026-11-26", 221)]);
        assert_eq!(flask.haystack, "pallets/flask the python micro framework for building web applications.");
        let quoted = named(&list, "tinyorg/pdfquote");
        assert_eq!(quoted.language, "");
        assert_eq!(quoted.description, r#"A "fast" PDF reader, with no language GitHub could name."#);
        assert_eq!(named(&list, "newowner/renamed-tool").history.days().count(), 3);
    }

    #[test]
    fn the_haystack_is_lowercased_beyond_ascii() {
        let text = format!("{FIXTURE}Ünïcode/ÉTÉ\tRust\t5\t2026-11-26\t1\t2026-11-26:1\tÇA VA\n");
        assert_eq!(named(&parse(&text).unwrap(), "Ünïcode/ÉTÉ").haystack, "ünïcode/été ça va");
    }

    #[test]
    fn a_line_ending_in_a_carriage_return_is_read_as_without_it() {
        let crlf = parse(&FIXTURE.replace('\n', "\r\n")).unwrap();
        let lf = parse(FIXTURE).unwrap();
        assert!(crlf.repos().eq(lf.repos()));
    }

    #[test]
    fn refuses_another_format() {
        assert_eq!(line_of(parse(&FIXTURE.replacen("format 1", "format 2", 1))), 1);
        assert_eq!(line_of(parse("")), 1);
        assert_eq!(line_of(parse(&FIXTURE.replacen("full_name\tlanguage", "full_name\tlang", 1))), 2);
    }

    #[test]
    fn refuses_a_line_with_the_wrong_number_of_fields() {
        let error = parse(&format!("{FIXTURE}someone/thing\tRust\t5\n")).err().unwrap();
        assert_eq!(error.line, 8);
        assert!(error.to_string().starts_with("line 8: "), "{error}");
    }

    #[test]
    fn refuses_a_figure_a_day_or_a_history_it_cannot_read() {
        let line = |stars: &str, day: &str, history: &str| {
            format!("{FIXTURE}someone/thing\tRust\t{stars}\t{day}\t1\t{history}\tA thing.\n")
        };
        for bad in ["5x", "-1"] {
            assert_eq!(line_of(parse(&line(bad, "2026-11-26", "2026-11-26:1"))), 8, "stars {bad}");
        }
        for bad in ["26 November", "2026-11-2x", ""] {
            assert_eq!(line_of(parse(&line("5", bad, "2026-11-26:1"))), 8, "day {bad:?}");
        }
        for bad in ["", "2026-11-26", "2026-11-26:x", "2026-11-26:1,", "yesterday:1"] {
            assert_eq!(line_of(parse(&line("5", "2026-11-26", bad))), 8, "history {bad:?}");
        }
    }

    #[test]
    fn sorts_newest_board_day_then_rank() {
        let list = parse(FIXTURE).unwrap();
        let order: Vec<&str> = list.repos().map(|repo| repo.full_name).collect();
        assert_eq!(
            order,
            ["pdf-labs/PDF-Kit", "tinyorg/pdfquote", "pallets/flask", "newowner/renamed-tool", "acme/old-days"]
        );
    }
}
