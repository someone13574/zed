use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use assets::Assets;
use client::{Client, UserStore};
use editor::Editor;
use fs::{FakeFs, Fs};
use gpui::{App, AppContext as _};
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use project_panel::ProjectPanel;
use session::{AppSession, Session};
use settings::{KeymapFile, DEFAULT_KEYMAP_PATH};
use theme::LoadThemes;
use util::ResultExt;
use uuid::Uuid;
use workspace::{AppState, OpenOptions, Workspace, WorkspaceStore};

// Poll a future exactly once with a no-op waker. On WASM all futures we need
// to block on are synchronous (all stubs return Poll::Ready immediately), so
// this is safe. Using futures::executor::block_on would register an executor
// context and prevent the nested block_on calls in KEY_VALUE_STORE's lazy init.
fn poll_ready_sync<F: Future>(future: F) -> F::Output {
    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |data| RawWaker::new(data, &VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("WASM sync future was not immediately ready"),
    }
}

fn init_app_state(cx: &mut App) -> Arc<AppState> {
    let fs: Arc<dyn Fs> = FakeFs::new(cx.background_executor().clone());
    <dyn Fs>::set_global(fs.clone(), cx);

    let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
    let client = Client::production(cx);
    let user_store = cx.new(|cx| UserStore::new(client.clone(), cx));
    let workspace_store = cx.new(|cx| WorkspaceStore::new(client.clone(), cx));
    // Session::new is synchronous on WASM (all DB ops are no-op stubs returning
    // Poll::Ready immediately). We poll manually to avoid registering an executor
    // context, which would prevent the nested futures::executor::block_on in
    // KEY_VALUE_STORE's lazy initializer from running.
    let session_value = poll_ready_sync(Session::new(Uuid::new_v4().to_string()));
    let session = cx.new(|cx| AppSession::new(session_value, cx));

    Client::set_global(client.clone(), cx);
    client::init(&client, cx);

    let app_state = Arc::new(AppState {
        client,
        fs,
        languages,
        user_store,
        workspace_store,
        node_runtime: NodeRuntime::unavailable(),
        build_window_options: |_, _| Default::default(),
        session,
    });
    AppState::set_global(Arc::downgrade(&app_state), cx);
    app_state
}

fn run_web_app() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
        settings::init(cx);
        theme::init(LoadThemes::All(Box::new(Assets)), cx);
        Assets.load_fonts(cx).expect("failed to load fonts");

        cx.bind_keys(
            KeymapFile::load_asset_allow_partial_failure(DEFAULT_KEYMAP_PATH, cx)
                .expect("failed to load default keymap"),
        );

        let app_state = init_app_state(cx);

        project::Project::init(&app_state.client, cx);
        workspace::init(app_state.clone(), cx);
        editor::init(cx);
        go_to_line::init(cx);
        project_panel::init(cx);

        cx.observe_new(|workspace: &mut Workspace, window, cx| {
            let Some(window) = window else { return };
            let cursor_position =
                cx.new(|_| go_to_line::cursor_position::CursorPosition::new(workspace));
            workspace.status_bar().update(cx, |status_bar, cx| {
                status_bar.add_right_item(cursor_position, window, cx);
            });

            let fs = workspace.app_state().fs.clone();
            workspace.set_prompt_for_open_path(Box::new(move |_workspace, _lister, window, cx| {
                let fs = fs.clone();
                let (tx, rx) = futures::channel::oneshot::channel();
                cx.spawn_in(window, async move |_workspace_handle, _cx| {
                    let paths = match gpui_platform::pick_browser_files(true).await {
                        Some(files) if !files.is_empty() => {
                            let mut paths = Vec::new();
                            for (name, bytes) in files {
                                let path =
                                    std::path::PathBuf::from(format!("/uploads/{name}"));
                                fs.write(&path, &bytes).await.ok();
                                paths.push(path);
                            }
                            Some(paths)
                        }
                        _ => None,
                    };
                    tx.send(paths).ok();
                })
                .detach();
                rx
            }));

            cx.spawn_in(window, async move |workspace_handle, cx| {
                if let Some(panel) = ProjectPanel::load(workspace_handle.clone(), cx.clone())
                    .await
                    .log_err()
                {
                    workspace_handle
                        .update_in(cx, |workspace, window, cx| {
                            workspace.add_panel(panel, window, cx);
                        })
                        .log_err();
                }
            })
            .detach();
        })
        .detach();

        let open_workspace_task =
            workspace::open_new(OpenOptions::default(), app_state, cx, |workspace, window, cx| {
                Editor::new_in_workspace(workspace, window, cx).detach();
            });
        cx.spawn(async move |_cx| {
            if let Err(error) = open_workspace_task.await {
                eprintln!("failed to open workspace in web bootstrap: {error:#}");
            }
        })
        .detach();

        cx.activate(true);
    });
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_web_app();
}
