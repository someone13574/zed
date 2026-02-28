use anyhow::{Result, anyhow};
use collections::HashMap;
use futures::Future;
use gpui::{App, AppContext as _, AsyncApp, SharedString, Task};
use parking_lot::{Mutex, RwLock};
use schemars::JsonSchema;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering::SeqCst};
use std::time::Duration;
use util::ConnectionResult;

pub use lsp_types::request::*;
pub use lsp_types::*;

pub const DEFAULT_LSP_REQUEST_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_LSP_REQUEST_TIMEOUT: Duration =
    Duration::from_secs(DEFAULT_LSP_REQUEST_TIMEOUT_SECS);

#[derive(Debug, Clone, Copy)]
pub enum IoKind {
    StdOut,
    StdIn,
    StdErr,
}

#[derive(Clone)]
pub struct LanguageServerBinary {
    pub path: PathBuf,
    pub arguments: Vec<OsString>,
    pub env: Option<HashMap<String, String>>,
}

impl Serialize for LanguageServerBinary {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut structure = serializer.serialize_struct("LanguageServerBinary", 3)?;
        let arguments = self
            .arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        structure.serialize_field("path", &self.path)?;
        structure.serialize_field("arguments", &arguments)?;
        structure.serialize_field("env", &self.env)?;
        structure.end()
    }
}

#[derive(Debug, Clone)]
pub struct LanguageServerBinaryOptions {
    pub allow_path_lookup: bool,
    pub allow_binary_download: bool,
    pub pre_release: bool,
}

pub struct LanguageServer {
    server_id: LanguageServerId,
    name: LanguageServerName,
    version: Option<SharedString>,
    process_name: Arc<str>,
    binary: LanguageServerBinary,
    capabilities: RwLock<ServerCapabilities>,
    configuration: Arc<DidChangeConfigurationParams>,
    code_action_kinds: Option<Vec<CodeActionKind>>,
    workspace_folders: Option<Arc<Mutex<BTreeSet<Uri>>>>,
    root_uri: Uri,
    next_id: AtomicI32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LanguageServerSelector {
    Id(LanguageServerId),
    Name(LanguageServerName),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct LanguageServerId(pub usize);

impl LanguageServerId {
    pub fn from_proto(id: u64) -> Self {
        Self(id as usize)
    }

    pub fn to_proto(self) -> u64 {
        self.0 as u64
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, JsonSchema,
)]
#[serde(transparent)]
pub struct LanguageServerName(pub SharedString);

impl std::fmt::Display for LanguageServerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for LanguageServerName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl AsRef<OsStr> for LanguageServerName {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref().as_ref()
    }
}

impl LanguageServerName {
    pub const fn new_static(s: &'static str) -> Self {
        Self(SharedString::new_static(s))
    }

    pub fn from_proto(s: String) -> Self {
        Self(s.into())
    }
}

impl<'a> From<&'a str> for LanguageServerName {
    fn from(string: &'a str) -> LanguageServerName {
        LanguageServerName(string.to_string().into())
    }
}

impl PartialEq<str> for LanguageServerName {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

pub enum Subscription {
    Notification,
    Io,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Int(i32),
    Str(String),
}

pub trait LspRequestFuture<O>: Future<Output = ConnectionResult<O>> {
    fn id(&self) -> i32;
}

struct LspRequest<F> {
    id: i32,
    request: F,
}

impl<F> LspRequest<F> {
    fn new(id: i32, request: F) -> Self {
        Self { id, request }
    }
}

impl<F: Future> Future for LspRequest<F> {
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // SAFETY: This is standard pin projection; the outer struct is pinned.
        let inner = unsafe { std::pin::Pin::new_unchecked(&mut self.get_unchecked_mut().request) };
        inner.poll(cx)
    }
}

impl<F, O> LspRequestFuture<O> for LspRequest<F>
where
    F: Future<Output = ConnectionResult<O>>,
{
    fn id(&self) -> i32 {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct AdapterServerCapabilities {
    pub server_capabilities: ServerCapabilities,
    pub code_action_kinds: Option<Vec<CodeActionKind>>,
}

pub const SEMANTIC_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::CLASS,
    SemanticTokenType::ENUM,
    SemanticTokenType::INTERFACE,
    SemanticTokenType::STRUCT,
    SemanticTokenType::TYPE_PARAMETER,
    SemanticTokenType::TYPE,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::ENUM_MEMBER,
    SemanticTokenType::DECORATOR,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::METHOD,
    SemanticTokenType::MACRO,
    SemanticTokenType::new("label"),
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::NUMBER,
    SemanticTokenType::REGEXP,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::MODIFIER,
    SemanticTokenType::EVENT,
    SemanticTokenType::new("lifetime"),
];

pub const SEMANTIC_TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::DEFINITION,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::STATIC,
    SemanticTokenModifier::DEPRECATED,
    SemanticTokenModifier::ABSTRACT,
    SemanticTokenModifier::ASYNC,
    SemanticTokenModifier::MODIFICATION,
    SemanticTokenModifier::DOCUMENTATION,
    SemanticTokenModifier::DEFAULT_LIBRARY,
    SemanticTokenModifier::new("constant"),
];

impl LanguageServer {
    pub fn new(
        _stderr_capture: Arc<Mutex<Option<String>>>,
        server_id: LanguageServerId,
        server_name: LanguageServerName,
        binary: LanguageServerBinary,
        root_path: &Path,
        code_action_kinds: Option<Vec<CodeActionKind>>,
        workspace_folders: Option<Arc<Mutex<BTreeSet<Uri>>>>,
        _cx: &mut AsyncApp,
    ) -> Result<Self> {
        let root_uri = Uri::from_file_path(root_path)
            .or_else(|_| Uri::from_str("file:///"))
            .map_err(|error| anyhow!("invalid root uri: {error}"))?;

        Ok(Self {
            server_id,
            name: server_name,
            version: None,
            process_name: Arc::<str>::from("wasm-lsp"),
            binary,
            capabilities: RwLock::new(Self::full_capabilities()),
            configuration: Arc::new(DidChangeConfigurationParams {
                settings: Value::Null,
            }),
            code_action_kinds,
            workspace_folders,
            root_uri,
            next_id: AtomicI32::new(1),
        })
    }

    pub fn code_action_kinds(&self) -> Option<Vec<CodeActionKind>> {
        self.code_action_kinds.clone()
    }

    pub fn default_initialize_params(
        &self,
        _pull_diagnostics: bool,
        _augments_syntax_tokens: bool,
        _cx: &App,
    ) -> InitializeParams {
        InitializeParams {
            root_uri: Some(self.root_uri.clone()),
            ..InitializeParams::default()
        }
    }

    pub fn initialize(
        mut self,
        _params: InitializeParams,
        configuration: Arc<DidChangeConfigurationParams>,
        _timeout: Duration,
        cx: &App,
    ) -> Task<Result<Arc<Self>>> {
        self.configuration = configuration;
        cx.background_spawn(async move { Ok(Arc::new(self)) })
    }

    pub fn shutdown(&self) -> Option<std::future::Ready<Option<()>>> {
        None
    }

    #[must_use]
    pub fn on_notification<T, F>(&self, _callback: F) -> Subscription
    where
        T: notification::Notification,
        F: 'static + Send + FnMut(T::Params, &mut AsyncApp),
    {
        Subscription::Notification
    }

    #[must_use]
    pub fn on_request<T, F, Fut>(&self, _callback: F) -> Subscription
    where
        T: request::Request,
        T::Params: 'static + Send,
        F: 'static + FnMut(T::Params, &mut AsyncApp) -> Fut + Send,
        Fut: 'static + Future<Output = Result<T::Result>>,
    {
        Subscription::Notification
    }

    #[must_use]
    pub fn on_io<F>(&self, _callback: F) -> Subscription
    where
        F: 'static + Send + FnMut(IoKind, &str),
    {
        Subscription::Io
    }

    pub fn remove_request_handler<T: request::Request>(&self) {
        let _ = std::marker::PhantomData::<T>;
    }

    pub fn remove_notification_handler<T: notification::Notification>(&self) {
        let _ = std::marker::PhantomData::<T>;
    }

    pub fn has_notification_handler<T: notification::Notification>(&self) -> bool {
        let _ = std::marker::PhantomData::<T>;
        false
    }

    pub fn name(&self) -> LanguageServerName {
        self.name.clone()
    }

    pub fn version(&self) -> Option<SharedString> {
        self.version.clone()
    }

    pub fn process_name(&self) -> &str {
        &self.process_name
    }

    pub fn capabilities(&self) -> ServerCapabilities {
        self.capabilities.read().clone()
    }

    pub fn adapter_server_capabilities(&self) -> AdapterServerCapabilities {
        AdapterServerCapabilities {
            server_capabilities: self.capabilities(),
            code_action_kinds: self.code_action_kinds(),
        }
    }

    pub fn update_capabilities(&self, update: impl FnOnce(&mut ServerCapabilities)) {
        update(&mut self.capabilities.write());
    }

    pub fn configuration(&self) -> &Value {
        &self.configuration.settings
    }

    pub fn server_id(&self) -> LanguageServerId {
        self.server_id
    }

    pub fn process_id(&self) -> Option<u32> {
        None
    }

    pub fn binary(&self) -> &LanguageServerBinary {
        &self.binary
    }

    pub fn request<T: request::Request>(
        &self,
        _params: T::Params,
        _request_timeout: Duration,
    ) -> impl LspRequestFuture<T::Result>
    where
        T::Result: 'static + Send,
    {
        let id = self.next_id.fetch_add(1, SeqCst);
        LspRequest::new(id, async move {
            ConnectionResult::Result(Err(anyhow!(
                "language server request is unavailable on wasm"
            )))
        })
    }

    pub fn request_with_timer<T: request::Request, U: Future<Output = String>>(
        &self,
        params: T::Params,
        _timer: U,
    ) -> impl LspRequestFuture<T::Result>
    where
        T::Result: 'static + Send,
    {
        self.request::<T>(params, Duration::ZERO)
    }

    pub fn request_timer(&self, timeout: Duration) -> impl Future<Output = String> {
        async move { format!("request timed out after {timeout:?}") }
    }

    pub fn notify<T: notification::Notification>(&self, _params: T::Params) -> Result<()> {
        Ok(())
    }

    pub fn add_workspace_folder(&self, uri: Uri) {
        if let Some(workspace_folders) = self.workspace_folders.as_ref() {
            workspace_folders.lock().insert(uri);
        }
    }

    pub fn remove_workspace_folder(&self, uri: Uri) {
        if let Some(workspace_folders) = self.workspace_folders.as_ref() {
            workspace_folders.lock().remove(&uri);
        }
    }

    pub fn set_workspace_folders(&self, folders: BTreeSet<Uri>) {
        if let Some(workspace_folders) = self.workspace_folders.as_ref() {
            *workspace_folders.lock() = folders;
        }
    }

    pub fn workspace_folders(&self) -> BTreeSet<Uri> {
        self.workspace_folders.as_ref().map_or_else(
            || BTreeSet::from_iter([self.root_uri.clone()]),
            |folders| folders.lock().clone(),
        )
    }

    pub fn register_buffer(
        &self,
        _uri: Uri,
        _language_id: String,
        _version: i32,
        _initial_text: String,
    ) {
    }

    pub fn unregister_buffer(&self, _uri: Uri) {}

    pub fn full_capabilities() -> ServerCapabilities {
        ServerCapabilities::default()
    }
}

impl Subscription {
    pub fn detach(&mut self) {}
}

impl fmt::Display for LanguageServerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Debug for LanguageServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LanguageServer")
            .field("id", &self.server_id.0)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for LanguageServerBinary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("LanguageServerBinary");
        debug.field("path", &self.path);
        debug.field("arguments", &self.arguments);
        if self.env.is_some() {
            debug.field("env", &"<redacted>");
        }
        debug.finish()
    }
}
