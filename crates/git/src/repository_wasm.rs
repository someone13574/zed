use crate::blame::Blame;
use crate::stash::GitStash;
use crate::status::{DiffTreeType, GitStatus, TreeDiff};
use crate::{Oid, RunHook, SHORT_SHA_LENGTH};
use anyhow::{Context as _, Result};
use collections::HashMap;
use futures::FutureExt as _;
use futures::future::BoxFuture;
use gpui::{AsyncApp, SharedString, Task};
use rope::Rope;
use schemars::JsonSchema;
use serde::Deserialize;
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::Arc;
use sum_tree::MapSeekTarget;
use text::LineEnding;
use util::normalize_path;
use util::paths::PathStyle;
use util::rel_path::RelPath;

pub type Sender<T> = async_channel::Sender<T>;

pub const REMOTE_CANCELLED_BY_USER: &str = "Operation cancelled by user";

pub const GRAPH_CHUNK_SIZE: usize = 1000;
pub const DEFAULT_WORKTREE_DIRECTORY: &str = "../worktrees";

pub fn resolve_worktree_directory(
    working_directory: &Path,
    worktree_directory_setting: &str,
) -> PathBuf {
    let trimmed = worktree_directory_setting.trim_end_matches(['/', '\\']);
    let joined = working_directory.join(trimmed);
    let resolved = normalize_path(&joined);

    if resolved.starts_with(working_directory) {
        resolved
    } else if let Some(repository_directory_name) = working_directory.file_name() {
        resolved.join(repository_directory_name)
    } else {
        resolved
    }
}

pub fn validate_worktree_directory(
    working_directory: &Path,
    worktree_directory_setting: &str,
) -> Result<PathBuf> {
    if Path::new(worktree_directory_setting).is_absolute()
        || worktree_directory_setting.starts_with('/')
        || worktree_directory_setting.starts_with('\\')
    {
        anyhow::bail!(
            "git.worktree_directory must be a relative path, got: {worktree_directory_setting:?}"
        );
    }

    if worktree_directory_setting.is_empty() {
        anyhow::bail!("git.worktree_directory must not be empty");
    }

    let trimmed = worktree_directory_setting.trim_end_matches(['/', '\\']);
    if trimmed == ".." {
        anyhow::bail!("git.worktree_directory must not be \"..\" (use \"../some-name\" instead)");
    }

    let resolved = resolve_worktree_directory(working_directory, worktree_directory_setting);
    let parent = working_directory.parent().unwrap_or(working_directory);

    if !resolved.starts_with(parent) {
        anyhow::bail!(
            "git.worktree_directory resolved to {resolved:?}, which is outside \
             the project root and its parent directory. It must resolve to a \
             subdirectory of {working_directory:?} or a sibling of it."
        );
    }

    Ok(resolved)
}

pub fn worktree_path_for_branch(
    working_directory: &Path,
    worktree_directory_setting: &str,
    branch: &str,
) -> PathBuf {
    resolve_worktree_directory(working_directory, worktree_directory_setting).join(branch)
}

#[derive(Debug, Clone)]
pub struct GraphCommitData {
    pub sha: Oid,
    pub parents: SmallVec<[Oid; 1]>,
    pub author_name: SharedString,
    pub author_email: SharedString,
    pub commit_timestamp: i64,
    pub subject: SharedString,
}

#[derive(Debug)]
pub struct InitialGraphCommitData {
    pub sha: Oid,
    pub parents: SmallVec<[Oid; 1]>,
    pub ref_names: Vec<SharedString>,
}

#[derive(Debug, Default)]
pub struct CommitDataReader;

impl CommitDataReader {
    pub async fn read(&self, _sha: Oid) -> Result<GraphCommitData> {
        anyhow::bail!("commit data reader is not available in WASM")
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Branch {
    pub is_head: bool,
    pub ref_name: SharedString,
    pub upstream: Option<Upstream>,
    pub most_recent_commit: Option<CommitSummary>,
}

impl Branch {
    pub fn name(&self) -> &str {
        self.ref_name
            .as_ref()
            .strip_prefix("refs/heads/")
            .or_else(|| self.ref_name.as_ref().strip_prefix("refs/remotes/"))
            .unwrap_or(self.ref_name.as_ref())
    }

    pub fn is_remote(&self) -> bool {
        self.ref_name.starts_with("refs/remotes/")
    }

    pub fn remote_name(&self) -> Option<&str> {
        self.ref_name
            .strip_prefix("refs/remotes/")
            .and_then(|stripped| stripped.split('/').next())
    }

    pub fn tracking_status(&self) -> Option<UpstreamTrackingStatus> {
        self.upstream
            .as_ref()
            .and_then(|upstream| upstream.tracking.status())
    }

    pub fn priority_key(&self) -> (bool, Option<i64>) {
        (
            self.is_head,
            self.most_recent_commit
                .as_ref()
                .map(|commit| commit.commit_timestamp),
        )
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub ref_name: SharedString,
    pub sha: SharedString,
}

impl Worktree {
    pub fn branch(&self) -> &str {
        self.ref_name
            .as_ref()
            .strip_prefix("refs/heads/")
            .or_else(|| self.ref_name.as_ref().strip_prefix("refs/remotes/"))
            .unwrap_or(self.ref_name.as_ref())
    }
}

pub fn parse_worktrees_from_str<T: AsRef<str>>(raw_worktrees: T) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let normalized = raw_worktrees.as_ref().replace("\r\n", "\n");
    for entry in normalized.split("\n\n") {
        let mut path = None;
        let mut sha = None;
        let mut ref_name = None;

        for line in entry.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("worktree ") {
                path = Some(rest.to_string());
            } else if let Some(rest) = line.strip_prefix("HEAD ") {
                sha = Some(rest.to_string());
            } else if let Some(rest) = line.strip_prefix("branch ") {
                ref_name = Some(rest.to_string());
            }
        }

        if let (Some(path), Some(sha), Some(ref_name)) = (path, sha, ref_name) {
            worktrees.push(Worktree {
                path: PathBuf::from(path),
                ref_name: ref_name.into(),
                sha: sha.into(),
            });
        }
    }

    worktrees
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Upstream {
    pub ref_name: SharedString,
    pub tracking: UpstreamTracking,
}

impl Upstream {
    pub fn is_remote(&self) -> bool {
        self.remote_name().is_some()
    }

    pub fn remote_name(&self) -> Option<&str> {
        self.ref_name
            .strip_prefix("refs/remotes/")
            .and_then(|stripped| stripped.split('/').next())
    }

    pub fn stripped_ref_name(&self) -> Option<&str> {
        self.ref_name.strip_prefix("refs/remotes/")
    }

    pub fn branch_name(&self) -> Option<&str> {
        self.ref_name
            .strip_prefix("refs/remotes/")
            .and_then(|stripped| stripped.split_once('/').map(|(_, name)| name))
    }
}

#[derive(Clone, Copy, Default)]
pub struct CommitOptions {
    pub amend: bool,
    pub signoff: bool,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum UpstreamTracking {
    Gone,
    Tracked(UpstreamTrackingStatus),
}

impl From<UpstreamTrackingStatus> for UpstreamTracking {
    fn from(value: UpstreamTrackingStatus) -> Self {
        UpstreamTracking::Tracked(value)
    }
}

impl UpstreamTracking {
    pub fn is_gone(&self) -> bool {
        matches!(self, UpstreamTracking::Gone)
    }

    pub fn status(&self) -> Option<UpstreamTrackingStatus> {
        match self {
            UpstreamTracking::Gone => None,
            UpstreamTracking::Tracked(status) => Some(*status),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteCommandOutput {
    pub stdout: String,
    pub stderr: String,
}

impl RemoteCommandOutput {
    pub fn is_empty(&self) -> bool {
        self.stdout.is_empty() && self.stderr.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct UpstreamTrackingStatus {
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CommitSummary {
    pub sha: SharedString,
    pub subject: SharedString,
    pub commit_timestamp: i64,
    pub author_name: SharedString,
    pub has_parent: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct CommitDetails {
    pub sha: SharedString,
    pub message: SharedString,
    pub commit_timestamp: i64,
    pub author_email: SharedString,
    pub author_name: SharedString,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FileHistoryEntry {
    pub sha: SharedString,
    pub subject: SharedString,
    pub message: SharedString,
    pub commit_timestamp: i64,
    pub author_name: SharedString,
    pub author_email: SharedString,
}

#[derive(Debug, Clone)]
pub struct FileHistory {
    pub entries: Vec<FileHistoryEntry>,
    pub path: RepoPath,
}

#[derive(Debug)]
pub struct CommitDiff {
    pub files: Vec<CommitFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitFileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug)]
pub struct CommitFile {
    pub path: RepoPath,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub is_binary: bool,
}

impl CommitFile {
    pub fn status(&self) -> CommitFileStatus {
        match (&self.old_text, &self.new_text) {
            (None, Some(_)) => CommitFileStatus::Added,
            (Some(_), None) => CommitFileStatus::Deleted,
            _ => CommitFileStatus::Modified,
        }
    }
}

impl CommitDetails {
    pub fn short_sha(&self) -> SharedString {
        self.sha.chars().take(SHORT_SHA_LENGTH).collect::<String>().into()
    }
}

pub fn is_binary_content(content: &[u8]) -> bool {
    let check_len = content.len().min(8000);
    content[..check_len].contains(&0)
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Remote {
    pub name: SharedString,
}

pub enum ResetMode {
    Soft,
    Mixed,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum FetchOptions {
    All,
    Remote(Remote),
}

impl FetchOptions {
    pub fn to_proto(&self) -> Option<String> {
        match self {
            FetchOptions::All => None,
            FetchOptions::Remote(remote) => Some(remote.clone().name.into()),
        }
    }

    pub fn from_proto(remote_name: Option<String>) -> Self {
        match remote_name {
            Some(name) => FetchOptions::Remote(Remote { name: name.into() }),
            None => FetchOptions::All,
        }
    }

    pub fn name(&self) -> SharedString {
        match self {
            Self::All => "Fetch all remotes".into(),
            Self::Remote(remote) => remote.name.clone(),
        }
    }
}

impl std::fmt::Display for FetchOptions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchOptions::All => write!(formatter, "--all"),
            FetchOptions::Remote(remote) => write!(formatter, "{}", remote.name),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Copy)]
pub enum LogOrder {
    #[default]
    DateOrder,
    TopoOrder,
    AuthorDateOrder,
    ReverseChronological,
}

impl LogOrder {
    pub fn as_arg(&self) -> &'static str {
        match self {
            LogOrder::DateOrder => "--date-order",
            LogOrder::TopoOrder => "--topo-order",
            LogOrder::AuthorDateOrder => "--author-date-order",
            LogOrder::ReverseChronological => "--reverse",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum LogSource {
    #[default]
    All,
    Branch(SharedString),
    Sha(Oid),
}

impl LogSource {
    pub fn get_arg(&self) -> Result<&str> {
        match self {
            LogSource::All => Ok("--all"),
            LogSource::Branch(branch) => Ok(branch.as_str()),
            LogSource::Sha(_) => anyhow::bail!("sha log source is not available in WASM"),
        }
    }
}

#[derive(Clone, Default)]
pub struct AskPassDelegate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskPassResult {
    CancelledByUser,
    Timedout,
}

#[derive(Default)]
pub struct AskPassSession;

impl AskPassSession {
    pub async fn run(&mut self) -> AskPassResult {
        AskPassResult::CancelledByUser
    }
}

pub trait GitRepository: Send + Sync {
    fn reload_index(&self);

    fn load_index_text(&self, path: RepoPath) -> BoxFuture<'_, Option<String>>;
    fn load_committed_text(&self, path: RepoPath) -> BoxFuture<'_, Option<String>>;
    fn load_blob_content(&self, oid: Oid) -> BoxFuture<'_, Result<String>>;

    fn set_index_text(
        &self,
        path: RepoPath,
        content: Option<String>,
        env: Arc<HashMap<String, String>>,
        is_executable: bool,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn remote_url(&self, name: &str) -> BoxFuture<'_, Option<String>>;
    fn revparse_batch(&self, revs: Vec<String>) -> BoxFuture<'_, Result<Vec<Option<String>>>>;

    fn head_sha(&self) -> BoxFuture<'_, Option<String>> {
        async move {
            self.revparse_batch(vec!["HEAD".into()])
                .await
                .unwrap_or_default()
                .into_iter()
                .next()
                .flatten()
        }
        .boxed()
    }

    fn merge_message(&self) -> BoxFuture<'_, Option<String>>;
    fn status(&self, path_prefixes: &[RepoPath]) -> Task<Result<GitStatus>>;
    fn diff_tree(&self, request: DiffTreeType) -> BoxFuture<'_, Result<TreeDiff>>;

    fn stash_entries(&self) -> BoxFuture<'_, Result<GitStash>>;
    fn branches(&self) -> BoxFuture<'_, Result<Vec<Branch>>>;

    fn change_branch(&self, name: String) -> BoxFuture<'_, Result<()>>;
    fn create_branch(
        &self,
        name: String,
        base_branch: Option<String>,
    ) -> BoxFuture<'_, Result<()>>;
    fn rename_branch(&self, branch: String, new_name: String) -> BoxFuture<'_, Result<()>>;
    fn delete_branch(&self, name: String) -> BoxFuture<'_, Result<()>>;

    fn worktrees(&self) -> BoxFuture<'_, Result<Vec<Worktree>>>;

    fn create_worktree(
        &self,
        name: String,
        directory: PathBuf,
        from_commit: Option<String>,
    ) -> BoxFuture<'_, Result<()>>;

    fn remove_worktree(&self, path: PathBuf, force: bool) -> BoxFuture<'_, Result<()>>;
    fn rename_worktree(&self, old_path: PathBuf, new_path: PathBuf) -> BoxFuture<'_, Result<()>>;

    fn reset(
        &self,
        commit: String,
        mode: ResetMode,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn checkout_files(
        &self,
        commit: String,
        paths: Vec<RepoPath>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn show(&self, commit: String) -> BoxFuture<'_, Result<CommitDetails>>;
    fn load_commit(&self, commit: String, cx: AsyncApp) -> BoxFuture<'_, Result<CommitDiff>>;
    fn blame(
        &self,
        path: RepoPath,
        content: Rope,
        line_ending: LineEnding,
    ) -> BoxFuture<'_, Result<Blame>>;
    fn file_history(&self, path: RepoPath) -> BoxFuture<'_, Result<FileHistory>>;
    fn file_history_paginated(
        &self,
        path: RepoPath,
        skip: usize,
        limit: Option<usize>,
    ) -> BoxFuture<'_, Result<FileHistory>>;

    fn path(&self) -> PathBuf;
    fn main_repository_path(&self) -> PathBuf;

    fn stage_paths(
        &self,
        paths: Vec<RepoPath>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn unstage_paths(
        &self,
        paths: Vec<RepoPath>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn run_hook(
        &self,
        hook: RunHook,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn commit(
        &self,
        message: SharedString,
        name_and_email: Option<(SharedString, SharedString)>,
        options: CommitOptions,
        askpass: AskPassDelegate,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn stash_paths(
        &self,
        paths: Vec<RepoPath>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn stash_pop(
        &self,
        index: Option<usize>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn stash_apply(
        &self,
        index: Option<usize>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn stash_drop(
        &self,
        index: Option<usize>,
        env: Arc<HashMap<String, String>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn push(
        &self,
        branch_name: String,
        remote_branch_name: String,
        upstream_name: String,
        options: Option<PushOptions>,
        askpass: AskPassDelegate,
        env: Arc<HashMap<String, String>>,
        cx: AsyncApp,
    ) -> BoxFuture<'_, Result<RemoteCommandOutput>>;

    fn pull(
        &self,
        branch_name: Option<String>,
        upstream_name: String,
        rebase: bool,
        askpass: AskPassDelegate,
        env: Arc<HashMap<String, String>>,
        cx: AsyncApp,
    ) -> BoxFuture<'_, Result<RemoteCommandOutput>>;

    fn fetch(
        &self,
        fetch_options: FetchOptions,
        askpass: AskPassDelegate,
        env: Arc<HashMap<String, String>>,
        cx: AsyncApp,
    ) -> BoxFuture<'_, Result<RemoteCommandOutput>>;

    fn get_push_remote(&self, branch: String) -> BoxFuture<'_, Result<Option<Remote>>>;
    fn get_branch_remote(&self, branch: String) -> BoxFuture<'_, Result<Option<Remote>>>;
    fn get_all_remotes(&self) -> BoxFuture<'_, Result<Vec<Remote>>>;
    fn remove_remote(&self, name: String) -> BoxFuture<'_, Result<()>>;
    fn create_remote(&self, name: String, url: String) -> BoxFuture<'_, Result<()>>;

    fn check_for_pushed_commit(&self) -> BoxFuture<'_, Result<Vec<SharedString>>>;

    fn diff(&self, diff: DiffType) -> BoxFuture<'_, Result<String>>;
    fn diff_stat(
        &self,
        diff: DiffType,
    ) -> BoxFuture<'_, Result<HashMap<RepoPath, crate::status::DiffStat>>>;

    fn checkpoint(&self) -> BoxFuture<'static, Result<GitRepositoryCheckpoint>>;
    fn restore_checkpoint(&self, checkpoint: GitRepositoryCheckpoint) -> BoxFuture<'_, Result<()>>;

    fn compare_checkpoints(
        &self,
        left: GitRepositoryCheckpoint,
        right: GitRepositoryCheckpoint,
    ) -> BoxFuture<'_, Result<bool>>;

    fn diff_checkpoints(
        &self,
        base_checkpoint: GitRepositoryCheckpoint,
        target_checkpoint: GitRepositoryCheckpoint,
    ) -> BoxFuture<'_, Result<String>>;

    fn default_branch(
        &self,
        include_remote_name: bool,
    ) -> BoxFuture<'_, Result<Option<SharedString>>>;

    fn initial_graph_data(
        &self,
        log_source: LogSource,
        log_order: LogOrder,
        request_tx: Sender<Vec<Arc<InitialGraphCommitData>>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn commit_data_reader(&self) -> Result<CommitDataReader>;
}

pub enum DiffType {
    HeadToIndex,
    HeadToWorktree,
    MergeBase { base_ref: SharedString },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
pub enum PushOptions {
    SetUpstream,
    Force,
}

impl std::fmt::Debug for dyn GitRepository {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("dyn GitRepository<...>").finish()
    }
}

#[derive(Clone, Debug)]
pub struct GitRepositoryCheckpoint {
    pub commit_sha: Oid,
}

#[derive(Debug, Clone)]
pub struct GitCommitter {
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_git_committer(_cx: &AsyncApp) -> GitCommitter {
    GitCommitter {
        name: None,
        email: None,
    }
}

#[derive(Clone, Ord, Hash, PartialOrd, Eq, PartialEq)]
pub struct RepoPath(Arc<RelPath>);

impl std::fmt::Debug for RepoPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl RepoPath {
    pub fn new<S: AsRef<str> + ?Sized>(value: &S) -> Result<Self> {
        let rel_path = RelPath::unix(value.as_ref())?;
        Ok(Self::from_rel_path(rel_path))
    }

    pub fn from_std_path(path: &Path, path_style: PathStyle) -> Result<Self> {
        let rel_path = RelPath::new(path, path_style)?;
        Ok(Self::from_rel_path(&rel_path))
    }

    pub fn from_proto(proto: &str) -> Result<Self> {
        let rel_path = RelPath::from_proto(proto)?;
        Ok(Self(rel_path))
    }

    pub fn from_rel_path(path: &RelPath) -> RepoPath {
        Self(Arc::from(path))
    }

    pub fn as_std_path(&self) -> &Path {
        if self.is_empty() {
            Path::new(".")
        } else {
            self.0.as_std_path()
        }
    }
}

pub fn repo_path<S: AsRef<str> + ?Sized>(value: &S) -> RepoPath {
    let rel_path = RelPath::unix(value.as_ref()).expect("test path should be valid");
    RepoPath(rel_path.into())
}

impl AsRef<Arc<RelPath>> for RepoPath {
    fn as_ref(&self) -> &Arc<RelPath> {
        &self.0
    }
}

impl std::ops::Deref for RepoPath {
    type Target = RelPath;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct RepoPathDescendants<'a>(pub &'a RepoPath);

impl MapSeekTarget<RepoPath> for RepoPathDescendants<'_> {
    fn cmp_cursor(&self, key: &RepoPath) -> Ordering {
        if key.starts_with(self.0) {
            Ordering::Greater
        } else {
            self.0.cmp(key)
        }
    }
}
