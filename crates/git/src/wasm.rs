#[path = "blame_wasm.rs"]
pub mod blame;
#[path = "commit_wasm.rs"]
pub mod commit;
#[path = "hosting_provider.rs"]
pub mod hosting_provider;
#[path = "remote.rs"]
pub mod remote;
#[path = "repository_wasm.rs"]
pub mod repository;
#[path = "stash.rs"]
pub mod stash;
#[path = "status.rs"]
pub mod status;

pub use self::hosting_provider::*;
pub use self::remote::*;
use anyhow::{Context as _, Result};
use gpui::{Action, actions};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DOT_GIT: &str = ".git";
pub const GITIGNORE: &str = ".gitignore";
pub const FSMONITOR_DAEMON: &str = "fsmonitor--daemon";
pub const LFS_DIR: &str = "lfs";
pub const COMMIT_MESSAGE: &str = "COMMIT_EDITMSG";
pub const INDEX_LOCK: &str = "index.lock";
pub const REPO_EXCLUDE: &str = "info/exclude";

actions!(
    git,
    [
        ToggleStaged,
        StageRange,
        StageAndNext,
        UnstageAndNext,
        #[action(deprecated_aliases = ["editor::RevertSelectedHunks"])]
        Restore,
        #[action(deprecated_aliases = ["editor::ToggleGitBlame"])]
        Blame,
        FileHistory,
        StageFile,
        UnstageFile,
        StageAll,
        UnstageAll,
        StashAll,
        StashPop,
        StashApply,
        RestoreTrackedFiles,
        TrashUntrackedFiles,
        Uncommit,
        Push,
        PushTo,
        ForcePush,
        Pull,
        PullRebase,
        Fetch,
        FetchFrom,
        Commit,
        Amend,
        Signoff,
        Cancel,
        ExpandCommitEditor,
        GenerateCommitMessage,
        Init,
        OpenModifiedFiles,
        Clone,
        AddToGitignore,
    ]
);

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = git)]
#[serde(deny_unknown_fields)]
pub struct RenameBranch {
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Action)]
#[action(namespace = git, deprecated_aliases = ["editor::RevertFile"])]
#[serde(deny_unknown_fields)]
pub struct RestoreFile {
    #[serde(default)]
    pub skip_prompt: bool,
}

pub const SHORT_SHA_LENGTH: usize = 7;

#[derive(Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Default)]
pub struct Oid([u8; 20]);

impl Oid {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 20 {
            anyhow::bail!("expected 20 bytes for git oid, got {}", bytes.len());
        }

        let mut parsed = [0_u8; 20];
        parsed.copy_from_slice(bytes);
        Ok(Self(parsed))
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn random(rng: &mut impl rand::Rng) -> Self {
        let mut bytes = [0; 20];
        rng.fill(&mut bytes);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub fn display_short(&self) -> String {
        self.to_string().chars().take(SHORT_SHA_LENGTH).collect()
    }
}

impl FromStr for Oid {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.len() != 40 {
            anyhow::bail!("expected 40-character oid, got {}", trimmed.len());
        }

        let mut parsed = [0_u8; 20];
        for (index, pair) in trimmed.as_bytes().chunks_exact(2).enumerate() {
            let hex = std::str::from_utf8(pair).context("oid is not valid utf-8")?;
            parsed[index] = u8::from_str_radix(hex, 16).context("oid contains non-hex digit")?;
        }

        Ok(Self(parsed))
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for Oid {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Oid {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse::<Oid>().map_err(serde::de::Error::custom)
    }
}

impl From<Oid> for u32 {
    fn from(oid: Oid) -> Self {
        let mut prefix = [0_u8; 4];
        prefix.copy_from_slice(&oid.0[..4]);
        u32::from_ne_bytes(prefix)
    }
}

impl From<Oid> for usize {
    fn from(oid: Oid) -> Self {
        let mut prefix = [0_u8; 8];
        prefix.copy_from_slice(&oid.0[..8]);
        u64::from_ne_bytes(prefix) as usize
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Debug)]
pub enum RunHook {
    PreCommit,
}

impl RunHook {
    pub fn as_str(&self) -> &str {
        match self {
            Self::PreCommit => "pre-commit",
        }
    }

    pub fn to_proto(&self) -> i32 {
        *self as i32
    }

    pub fn from_proto(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::PreCommit),
            _ => None,
        }
    }
}
