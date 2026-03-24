use std::path::PathBuf;
use std::{fmt, fs};

use chrono::{DateTime, Utc};

/// Priority level for a backlog item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Priority {
    Normal,
    Important,
    Urgent,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Normal => Ok(()),
            Priority::Important => f.write_str("!"),
            Priority::Urgent => f.write_str("!!"),
        }
    }
}

/// A single backlog entry with metadata parsed from markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BacklogItem {
    pub(crate) text: String,
    pub(crate) tags: Vec<String>,
    pub(crate) priority: Priority,
    pub(crate) context: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) completed: bool,
}

impl BacklogItem {
    /// Render a backlog item back to its markdown line representation.
    fn to_line(&self) -> String {
        let checkbox = if self.completed { "- [x] " } else { "- [ ] " };

        let priority_prefix = match self.priority {
            Priority::Urgent => "!!",
            Priority::Important => "!",
            Priority::Normal => "",
        };

        // Rebuild inline text with tags already embedded
        let text_with_tags = &self.text;

        let comment = match &self.context {
            Some(ctx) => format!(" <!-- ctx:{} {} -->", ctx, self.created_at.to_rfc3339()),
            None => format!(" <!-- {} -->", self.created_at.to_rfc3339()),
        };

        format!(
            "{}{}{}{}",
            checkbox, priority_prefix, text_with_tags, comment
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BacklogError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid item index: {0}")]
    InvalidIndex(usize),
}

/// Persistent backlog stored as a markdown file.
#[derive(Debug)]
pub(crate) struct Backlog {
    path: PathBuf,
    items: Vec<BacklogItem>,
}

impl Backlog {
    /// Load a backlog from the given file path. Returns an empty backlog if the
    /// file does not exist.
    pub(crate) fn load(path: impl Into<PathBuf>) -> Result<Self, BacklogError> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path,
                items: Vec::new(),
            });
        }

        let content = fs::read_to_string(&path)?;
        let items = parse_items(&content);
        Ok(Self { path, items })
    }

    /// Write the backlog back to disk, creating parent directories if needed.
    pub(crate) fn save(&self) -> Result<(), BacklogError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out = String::from("# Backlog\n");
        for item in &self.items {
            out.push('\n');
            out.push_str(&item.to_line());
        }
        out.push('\n');

        fs::write(&self.path, &out)?;
        Ok(())
    }

    /// Add a new item, parsing tags and priority from the text.
    pub(crate) fn add(&mut self, text: &str, context: Option<&str>) {
        let (priority, rest) = parse_priority(text);
        let tags = extract_tags(rest);

        self.items.push(BacklogItem {
            text: rest.to_owned(),
            tags,
            priority,
            context: context.map(|s| s.to_owned()),
            created_at: Utc::now(),
            completed: false,
        });
    }

    /// Remove an item by index.
    pub(crate) fn remove(&mut self, index: usize) -> Result<BacklogItem, BacklogError> {
        if index >= self.items.len() {
            return Err(BacklogError::InvalidIndex(index));
        }
        Ok(self.items.remove(index))
    }

    /// Toggle the completed state of an item by index.
    pub(crate) fn toggle(&mut self, index: usize) -> Result<(), BacklogError> {
        let item = self
            .items
            .get_mut(index)
            .ok_or(BacklogError::InvalidIndex(index))?;
        item.completed = !item.completed;
        Ok(())
    }

    /// Return a slice of all items.
    pub(crate) fn items(&self) -> &[BacklogItem] { &self.items }
}

/// Extract `#tag` tokens from text.
fn extract_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'#' {
            // Must be at start or preceded by whitespace
            let at_boundary = i == 0 || bytes[i - 1] == b' ';
            if at_boundary {
                let start = i + 1;
                let mut end = start;
                while end < len && is_tag_char(bytes[end]) {
                    end += 1;
                }
                if end > start {
                    tags.push(text[start..end].to_owned());
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }

    tags
}

fn is_tag_char(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' || b == b'-' }

/// Parse leading `!!` or `!` priority prefix and return the remaining text.
fn parse_priority(text: &str) -> (Priority, &str) {
    if let Some(rest) = text.strip_prefix("!!") {
        (Priority::Urgent, rest)
    } else if let Some(rest) = text.strip_prefix('!') {
        (Priority::Important, rest)
    } else {
        (Priority::Normal, text)
    }
}

/// Parse all backlog item lines from file content.
fn parse_items(content: &str) -> Vec<BacklogItem> {
    let mut items = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        let (completed, after_checkbox) = if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [X] ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            (false, rest)
        } else {
            continue;
        };

        // Split off HTML comment metadata from end
        let (body, context, created_at) = parse_comment_metadata(after_checkbox);

        let (priority, text) = parse_priority(body.trim());
        let tags = extract_tags(text);

        items.push(BacklogItem {
            text: text.to_owned(),
            tags,
            priority,
            context,
            created_at,
            completed,
        });
    }

    items
}

/// Extract optional `<!-- ctx:name ISO8601 -->` or `<!-- ISO8601 -->` from the
/// end of a line. Returns `(body, context, timestamp)`.
fn parse_comment_metadata(text: &str) -> (&str, Option<String>, DateTime<Utc>) {
    let fallback = || (text, None, Utc::now());

    let Some(start) = text.rfind("<!--") else {
        return fallback();
    };
    let Some(end) = text[start..].find("-->") else {
        return fallback();
    };

    let comment_inner = text[start + 4..start + end].trim();
    let body = text[..start].trim_end();

    if let Some(rest) = comment_inner.strip_prefix("ctx:") {
        // Format: ctx:<context> <ISO8601>
        if let Some(space_pos) = rest.rfind(' ') {
            let ctx = &rest[..space_pos];
            let ts_str = &rest[space_pos + 1..];
            if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                return (body, Some(ctx.to_owned()), ts);
            }
        }
    }

    // Format: just ISO8601
    if let Ok(ts) = comment_inner.parse::<DateTime<Utc>>() {
        return (body, None, ts);
    }

    fallback()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tags_from_text() {
        let tags = extract_tags("toast notifications #ui #perf feel sluggish");
        assert_eq!(tags, vec!["ui", "perf"]);
    }

    #[test]
    fn extract_tags_empty() {
        let tags = extract_tags("no tags here");
        assert!(tags.is_empty());
    }

    #[test]
    fn extract_tags_with_hyphens_and_underscores() {
        let tags = extract_tags("#my-tag #another_tag");
        assert_eq!(tags, vec!["my-tag", "another_tag"]);
    }

    #[test]
    fn extract_tags_ignores_mid_word_hash() {
        let tags = extract_tags("foo#bar #real");
        assert_eq!(tags, vec!["real"]);
    }

    #[test]
    fn parse_priority_urgent() {
        let (p, rest) = parse_priority("!!fix the bug");
        assert_eq!(p, Priority::Urgent);
        assert_eq!(rest, "fix the bug");
    }

    #[test]
    fn parse_priority_important() {
        let (p, rest) = parse_priority("!add feature");
        assert_eq!(p, Priority::Important);
        assert_eq!(rest, "add feature");
    }

    #[test]
    fn parse_priority_normal() {
        let (p, rest) = parse_priority("just a task");
        assert_eq!(p, Priority::Normal);
        assert_eq!(rest, "just a task");
    }

    #[test]
    fn parse_single_item_with_context() {
        let content = "# Backlog\n\n- [ ] !!toast notifications #ui #perf <!-- ctx:pr-babysitter 2026-03-25T12:00:00Z -->\n";
        let items = parse_items(content);

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.text, "toast notifications #ui #perf");
        assert_eq!(item.tags, vec!["ui", "perf"]);
        assert_eq!(item.priority, Priority::Urgent);
        assert_eq!(item.context.as_deref(), Some("pr-babysitter"));
        assert!(!item.completed);
    }

    #[test]
    fn parse_completed_item_without_context() {
        let content = "- [x] wire session creation modes <!-- 2026-03-24T10:00:00Z -->\n";
        let items = parse_items(content);

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.text, "wire session creation modes");
        assert!(item.tags.is_empty());
        assert_eq!(item.priority, Priority::Normal);
        assert!(item.context.is_none());
        assert!(item.completed);
    }

    #[test]
    fn parse_multiple_items() {
        let content = "\
# Backlog

- [ ] !!toast notifications #ui #perf <!-- ctx:pr-babysitter 2026-03-25T12:00:00Z -->
- [x] wire session creation modes <!-- 2026-03-24T10:00:00Z -->
- [ ] !add retry logic #infra <!-- ctx:engine 2026-03-23T08:00:00Z -->
";
        let items = parse_items(content);
        assert_eq!(items.len(), 3);

        assert_eq!(items[0].priority, Priority::Urgent);
        assert!(items[1].completed);
        assert_eq!(items[2].priority, Priority::Important);
        assert_eq!(items[2].context.as_deref(), Some("engine"));
        assert_eq!(items[2].tags, vec!["infra"]);
    }

    #[test]
    fn round_trip_preserves_content() {
        let original = "\
# Backlog

- [ ] !!toast notifications #ui #perf <!-- ctx:pr-babysitter 2026-03-25T12:00:00+00:00 -->
- [x] wire session creation modes <!-- 2026-03-24T10:00:00+00:00 -->
";
        let items = parse_items(original);
        assert_eq!(items.len(), 2);

        let mut out = String::from("# Backlog\n");
        for item in &items {
            out.push('\n');
            out.push_str(&item.to_line());
        }
        out.push('\n');

        assert_eq!(out, original);
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        assert!(backlog.items().is_empty());
    }

    #[test]
    fn add_parses_tags_and_priority() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        backlog.add("!!fix slow render #ui #perf", Some("dashboard"));

        assert_eq!(backlog.items().len(), 1);
        let item = &backlog.items()[0];
        assert_eq!(item.text, "fix slow render #ui #perf");
        assert_eq!(item.tags, vec!["ui", "perf"]);
        assert_eq!(item.priority, Priority::Urgent);
        assert_eq!(item.context.as_deref(), Some("dashboard"));
        assert!(!item.completed);
    }

    #[test]
    fn add_normal_priority_no_context() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        backlog.add("update docs", None);

        let item = &backlog.items()[0];
        assert_eq!(item.priority, Priority::Normal);
        assert!(item.context.is_none());
    }

    #[test]
    fn toggle_flips_completed() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        backlog.add("task one", None);

        assert!(!backlog.items()[0].completed);
        backlog.toggle(0).unwrap();
        assert!(backlog.items()[0].completed);
        backlog.toggle(0).unwrap();
        assert!(!backlog.items()[0].completed);
    }

    #[test]
    fn toggle_invalid_index() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        let result = backlog.toggle(0);
        assert!(matches!(result, Err(BacklogError::InvalidIndex(0))));
    }

    #[test]
    fn remove_item() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        backlog.add("first", None);
        backlog.add("second", None);

        let removed = backlog.remove(0).unwrap();
        assert_eq!(removed.text, "first");
        assert_eq!(backlog.items().len(), 1);
        assert_eq!(backlog.items()[0].text, "second");
    }

    #[test]
    fn remove_invalid_index() {
        let mut backlog = Backlog::load("/tmp/fridi-test-nonexistent-backlog.md").unwrap();
        let result = backlog.remove(5);
        assert!(matches!(result, Err(BacklogError::InvalidIndex(5))));
    }

    #[test]
    fn save_and_reload() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("backlog.md");

        let mut backlog = Backlog::load(&path).unwrap();
        backlog.add("!!urgent thing #core", Some("engine"));
        backlog.add("normal thing", None);
        backlog.save().unwrap();

        let reloaded = Backlog::load(&path).unwrap();
        assert_eq!(reloaded.items().len(), 2);

        let first = &reloaded.items()[0];
        assert_eq!(first.priority, Priority::Urgent);
        assert_eq!(first.text, "urgent thing #core");
        assert_eq!(first.tags, vec!["core"]);
        assert_eq!(first.context.as_deref(), Some("engine"));
        assert!(!first.completed);

        let second = &reloaded.items()[1];
        assert_eq!(second.priority, Priority::Normal);
        assert_eq!(second.text, "normal thing");
        assert!(second.context.is_none());
    }

    #[test]
    fn save_and_reload_with_toggle() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("backlog.md");

        let mut backlog = Backlog::load(&path).unwrap();
        backlog.add("task a", None);
        backlog.add("task b", None);
        backlog.toggle(1).unwrap();
        backlog.save().unwrap();

        let reloaded = Backlog::load(&path).unwrap();
        assert!(!reloaded.items()[0].completed);
        assert!(reloaded.items()[1].completed);
    }

    #[test]
    fn priority_display() {
        assert_eq!(format!("{}", Priority::Normal), "");
        assert_eq!(format!("{}", Priority::Important), "!");
        assert_eq!(format!("{}", Priority::Urgent), "!!");
    }

    #[test]
    fn parse_ignores_non_item_lines() {
        let content = "\
# Backlog

Some random text here.

- [ ] real item <!-- 2026-03-25T00:00:00Z -->

Another paragraph.
";
        let items = parse_items(content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "real item");
    }

    #[test]
    fn parse_item_without_comment_uses_current_time() {
        let content = "- [ ] orphan item with no metadata\n";
        let items = parse_items(content);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "orphan item with no metadata");
        // Timestamp should be roughly now
        let diff = Utc::now() - items[0].created_at;
        assert!(diff.num_seconds() < 5);
    }
}
