use std::{
    ffi::OsStr,
    ops::ControlFlow,
    path::PathBuf,
};

use anyhow::Result;
use futures::channel::oneshot;
use gpui::{AsyncApp, BackgroundExecutor, Task};

use crate::EncryptedPassword;

#[derive(PartialEq, Eq)]
pub enum AskPassResult {
    CancelledByUser,
    Timedout,
}

pub struct AskPassDelegate;

impl AskPassDelegate {
    pub fn new(
        _cx: &mut AsyncApp,
        _password_prompt: impl Fn(String, oneshot::Sender<EncryptedPassword>, &mut AsyncApp)
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self
    }

    pub fn ask_password(&mut self, _prompt: String) -> Task<Option<EncryptedPassword>> {
        Task::ready(None)
    }
}

pub struct AskPassSession {
    askpass_task: PasswordProxy,
    _executor: BackgroundExecutor,
}

impl AskPassSession {
    pub async fn new(
        executor: BackgroundExecutor,
        _delegate: AskPassDelegate,
    ) -> Result<Self> {
        Ok(Self {
            askpass_task: PasswordProxy {
                askpass_script_path: PathBuf::from("askpass-unsupported"),
            },
            _executor: executor,
        })
    }

    pub async fn run(&mut self) -> AskPassResult {
        AskPassResult::CancelledByUser
    }

    pub fn script_path(&self) -> impl AsRef<OsStr> {
        self.askpass_task.script_path()
    }

    #[cfg(target_os = "windows")]
    pub fn get_password(&self) -> Option<EncryptedPassword> {
        None
    }
}

pub struct PasswordProxy {
    askpass_script_path: PathBuf,
}

impl PasswordProxy {
    pub async fn new(
        _get_password: Box<
            dyn FnMut(String) -> Task<ControlFlow<(), Result<EncryptedPassword>>>
                + 'static
                + Send
                + Sync,
        >,
        _executor: BackgroundExecutor,
    ) -> Result<Self> {
        Ok(Self {
            askpass_script_path: PathBuf::from("askpass-unsupported"),
        })
    }

    pub fn script_path(&self) -> impl AsRef<OsStr> {
        &self.askpass_script_path
    }
}

pub fn main(_socket: &str) {}

pub fn set_askpass_program(_path: PathBuf) {}
