use crate::{
    BuildCommitPermalinkParams, GitHostingProviderRegistry, GitRemote, Oid, parse_git_remote_url,
    status::StatusCode,
};
use anyhow::Result;
use collections::HashMap;
use gpui::SharedString;
use std::{path::Path, sync::Arc};

#[derive(Clone, Debug, Default)]
pub struct ParsedCommitMessage {
    pub message: SharedString,
    pub permalink: Option<url::Url>,
    pub pull_request: Option<crate::hosting_provider::PullRequest>,
    pub remote: Option<GitRemote>,
}

impl ParsedCommitMessage {
    pub fn parse(
        sha: String,
        message: String,
        remote_url: Option<&str>,
        provider_registry: Option<Arc<GitHostingProviderRegistry>>,
    ) -> Self {
        if let Some((hosting_provider, remote)) = provider_registry
            .and_then(|registry| remote_url.and_then(|url| parse_git_remote_url(registry, url)))
        {
            let pull_request = hosting_provider.extract_pull_request(&remote, &message);
            Self {
                message: message.into(),
                permalink: Some(
                    hosting_provider
                        .build_commit_permalink(&remote, BuildCommitPermalinkParams { sha: &sha }),
                ),
                pull_request,
                remote: Some(GitRemote {
                    host: hosting_provider,
                    owner: remote.owner.into(),
                    repo: remote.repo.into(),
                }),
            }
        } else {
            Self {
                message: message.into(),
                ..Default::default()
            }
        }
    }
}

pub async fn get_messages(_working_directory: &Path, shas: &[Oid]) -> Result<HashMap<Oid, String>> {
    let mut messages = HashMap::default();
    for sha in shas {
        messages.insert(*sha, String::new());
    }
    Ok(messages)
}

pub fn parse_git_diff_name_status(content: &str) -> impl Iterator<Item = (&str, StatusCode)> {
    let mut parts = content.split('\0');
    std::iter::from_fn(move || {
        loop {
            let status = parts.next()?;
            let path = parts.next()?;
            let parsed_status = match status {
                "M" => StatusCode::Modified,
                "A" => StatusCode::Added,
                "D" => StatusCode::Deleted,
                _ => continue,
            };
            return Some((path, parsed_status));
        }
    })
}
