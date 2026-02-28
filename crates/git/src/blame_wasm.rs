use crate::Oid;
use anyhow::Result;
use collections::HashMap;
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::Path};
use text::{LineEnding, Rope};
use time::OffsetDateTime;
use time::UtcOffset;
use time::macros::format_description;

#[derive(Debug, Clone, Default)]
pub struct Blame {
    pub entries: Vec<BlameEntry>,
    pub messages: HashMap<Oid, String>,
}

impl Blame {
    pub async fn for_path(
        _git_binary: &Path,
        _working_directory: &Path,
        _path: &crate::repository::RepoPath,
        _content: &Rope,
        _line_ending: LineEnding,
    ) -> Result<Self> {
        Ok(Self::default())
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct BlameEntry {
    pub sha: Oid,
    pub range: Range<u32>,
    pub original_line_number: u32,
    pub author: Option<String>,
    pub author_mail: Option<String>,
    pub author_time: Option<i64>,
    pub author_tz: Option<String>,
    pub committer_name: Option<String>,
    pub committer_email: Option<String>,
    pub committer_time: Option<i64>,
    pub committer_tz: Option<String>,
    pub summary: Option<String>,
    pub previous: Option<String>,
    pub filename: String,
}

impl BlameEntry {
    pub fn author_offset_date_time(&self) -> Result<OffsetDateTime> {
        if let (Some(author_time), Some(author_tz)) = (self.author_time, &self.author_tz) {
            let format = format_description!("[offset_hour][offset_minute]");
            let offset = UtcOffset::parse(author_tz, &format)?;
            let date_time_utc = OffsetDateTime::from_unix_timestamp(author_time)?;
            Ok(date_time_utc.to_offset(offset))
        } else {
            Ok(OffsetDateTime::now_utc())
        }
    }
}
