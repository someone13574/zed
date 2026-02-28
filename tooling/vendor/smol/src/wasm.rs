use std::future::Future;
use std::io::{self as std_io, Read as _, Seek as _, Write as _};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub use async_channel as channel;
pub use async_executor::{Executor, LocalExecutor, Task};
pub use async_lock as lock;
pub use futures_lite::{future, io, pin, prelude, ready, stream};

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    future::block_on(future)
}

pub struct Timer {
    future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
}

impl std::fmt::Debug for Timer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Timer { .. }")
    }
}

impl Timer {
    pub fn after(_duration: Duration) -> Self {
        Self {
            future: Box::pin(async {}),
        }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}

pub async fn unblock<T>(f: impl FnOnce() -> T + Send + 'static) -> T {
    f()
}

#[derive(Debug)]
pub struct Unblock<T>(T);

impl<T> Unblock<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: std_io::Read + Unpin> io::AsyncRead for Unblock<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std_io::Result<usize>> {
        Poll::Ready(self.0.read(buffer))
    }
}

impl<T: std_io::Write + Unpin> io::AsyncWrite for Unblock<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std_io::Result<usize>> {
        Poll::Ready(self.0.write(buffer))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std_io::Result<()>> {
        Poll::Ready(self.0.flush())
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std_io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl<T: std_io::Seek + Unpin> io::AsyncSeek for Unblock<T> {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        position: std_io::SeekFrom,
    ) -> Poll<std_io::Result<u64>> {
        Poll::Ready(self.0.seek(position))
    }
}

pub struct Async<T>(pub T);

pub mod fs {
    use super::*;
    use futures_lite::Stream;
    use std::path::Path;

    pub use std::fs::{DirEntry, Metadata, OpenOptions};

    #[derive(Debug)]
    pub struct File(pub std::fs::File);

    impl File {
        pub async fn open(path: impl AsRef<Path>) -> std_io::Result<Self> {
            std::fs::File::open(path).map(Self)
        }

        pub async fn create(path: impl AsRef<Path>) -> std_io::Result<Self> {
            std::fs::File::create(path).map(Self)
        }

        pub async fn sync_all(&self) -> std_io::Result<()> {
            self.0.sync_all()
        }
    }

    impl io::AsyncRead for File {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &mut [u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.read(buffer))
        }
    }

    impl io::AsyncWrite for File {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.write(buffer))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std_io::Result<()>> {
            Poll::Ready(self.0.flush())
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std_io::Result<()>> {
            self.poll_flush(cx)
        }
    }

    impl io::AsyncSeek for File {
        fn poll_seek(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            position: std_io::SeekFrom,
        ) -> Poll<std_io::Result<u64>> {
            Poll::Ready(self.0.seek(position))
        }
    }

    #[derive(Debug)]
    pub struct ReadDir {
        entries: std::vec::IntoIter<std_io::Result<DirEntry>>,
    }

    impl Stream for ReadDir {
        type Item = std_io::Result<DirEntry>;

        fn poll_next(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.entries.next())
        }
    }

    pub async fn read(path: impl AsRef<Path>) -> std_io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub async fn read_to_string(path: impl AsRef<Path>) -> std_io::Result<String> {
        std::fs::read_to_string(path)
    }

    pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std_io::Result<()> {
        std::fs::write(path, contents)
    }

    pub async fn metadata(path: impl AsRef<Path>) -> std_io::Result<Metadata> {
        std::fs::metadata(path)
    }

    pub async fn create_dir(path: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::create_dir(path)
    }

    pub async fn create_dir_all(path: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::create_dir_all(path)
    }

    pub async fn remove_file(path: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::remove_file(path)
    }

    pub async fn remove_dir(path: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::remove_dir(path)
    }

    pub async fn remove_dir_all(path: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std_io::Result<()> {
        std::fs::rename(from, to)
    }

    pub async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std_io::Result<u64> {
        std::fs::copy(from, to)
    }

    pub async fn canonicalize(path: impl AsRef<Path>) -> std_io::Result<std::path::PathBuf> {
        std::fs::canonicalize(path)
    }

    pub async fn read_link(path: impl AsRef<Path>) -> std_io::Result<std::path::PathBuf> {
        std::fs::read_link(path)
    }

    pub async fn read_dir(path: impl AsRef<Path>) -> std_io::Result<ReadDir> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            entries.push(entry);
        }
        Ok(ReadDir {
            entries: entries.into_iter(),
        })
    }
}

pub mod process {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;

    pub type Stdio = std::process::Stdio;

    #[derive(Debug)]
    pub struct Command(std::process::Command);

    #[derive(Debug)]
    pub struct Child {
        inner: std::process::Child,
        pub stdin: Option<ChildStdin>,
        pub stdout: Option<ChildStdout>,
        pub stderr: Option<ChildStderr>,
    }

    #[derive(Debug)]
    pub struct ChildStdin(pub std::process::ChildStdin);

    #[derive(Debug)]
    pub struct ChildStdout(pub std::process::ChildStdout);

    #[derive(Debug)]
    pub struct ChildStderr(pub std::process::ChildStderr);

    impl Command {
        pub fn new(program: impl AsRef<OsStr>) -> Self {
            Self(std::process::Command::new(program))
        }

        pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
            self.0.arg(arg);
            self
        }

        pub fn args<I, S>(&mut self, args: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: AsRef<OsStr>,
        {
            self.0.args(args);
            self
        }

        pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
            self.0.env(key, value);
            self
        }

        pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
        where
            I: IntoIterator<Item = (K, V)>,
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.0.envs(vars);
            self
        }

        pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
            self.0.env_remove(key);
            self
        }

        pub fn env_clear(&mut self) -> &mut Self {
            self.0.env_clear();
            self
        }

        pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
            self.0.current_dir(dir);
            self
        }

        pub fn stdin(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
            self.0.stdin(cfg.into());
            self
        }

        pub fn stdout(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
            self.0.stdout(cfg.into());
            self
        }

        pub fn stderr(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
            self.0.stderr(cfg.into());
            self
        }

        pub fn kill_on_drop(&mut self, _kill_on_drop: bool) -> &mut Self {
            self
        }

        pub fn spawn(&mut self) -> std_io::Result<Child> {
            let mut inner = self.0.spawn()?;
            Ok(Child {
                stdin: inner.stdin.take().map(ChildStdin),
                stdout: inner.stdout.take().map(ChildStdout),
                stderr: inner.stderr.take().map(ChildStderr),
                inner,
            })
        }

        pub async fn output(&mut self) -> std_io::Result<std::process::Output> {
            self.0.output()
        }

        pub async fn status(&mut self) -> std_io::Result<std::process::ExitStatus> {
            self.0.status()
        }
    }

    impl From<std::process::Command> for Command {
        fn from(command: std::process::Command) -> Self {
            Self(command)
        }
    }

    impl Child {
        pub fn kill(&mut self) -> std_io::Result<()> {
            self.inner.kill()
        }

        pub fn id(&self) -> u32 {
            self.inner.id()
        }

        pub async fn status(&mut self) -> std_io::Result<std::process::ExitStatus> {
            self.inner.wait()
        }

        pub fn try_status(&mut self) -> std_io::Result<Option<std::process::ExitStatus>> {
            self.inner.try_wait()
        }

        pub async fn output(mut self) -> std_io::Result<std::process::Output> {
            self.inner.wait_with_output()
        }

        pub async fn wait(&mut self) -> std_io::Result<std::process::ExitStatus> {
            self.inner.wait()
        }

        pub async fn wait_with_output(mut self) -> std_io::Result<std::process::Output> {
            self.inner.wait_with_output()
        }
    }

    impl io::AsyncWrite for ChildStdin {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.write(buffer))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std_io::Result<()>> {
            Poll::Ready(self.0.flush())
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std_io::Result<()>> {
            self.poll_flush(cx)
        }
    }

    impl io::AsyncRead for ChildStdout {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &mut [u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.read(buffer))
        }
    }

    impl io::AsyncRead for ChildStderr {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &mut [u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.read(buffer))
        }
    }
}

pub mod net {
    use super::*;
    use std::net::{SocketAddr, ToSocketAddrs};
    use std::path::Path;

    #[derive(Debug)]
    pub struct TcpListener(std::net::TcpListener);

    #[derive(Debug)]
    pub struct TcpStream(std::net::TcpStream);

    impl TcpListener {
        pub async fn bind(address: impl ToSocketAddrs) -> std_io::Result<Self> {
            std::net::TcpListener::bind(address).map(Self)
        }

        pub async fn accept(&self) -> std_io::Result<(TcpStream, SocketAddr)> {
            self.0.accept().map(|(stream, address)| (TcpStream(stream), address))
        }

        pub fn local_addr(&self) -> std_io::Result<SocketAddr> {
            self.0.local_addr()
        }
    }

    impl TcpStream {
        pub async fn connect(address: impl ToSocketAddrs) -> std_io::Result<Self> {
            std::net::TcpStream::connect(address).map(Self)
        }
    }

    impl io::AsyncRead for TcpStream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &mut [u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.read(buffer))
        }
    }

    impl io::AsyncWrite for TcpStream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<std_io::Result<usize>> {
            Poll::Ready(self.0.write(buffer))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std_io::Result<()>> {
            Poll::Ready(self.0.flush())
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std_io::Result<()>> {
            self.poll_flush(cx)
        }
    }

    pub mod unix {
        use super::*;
        use std::path::PathBuf;

        #[derive(Debug)]
        pub struct UnixListener;

        #[derive(Debug)]
        pub struct UnixStream;

        impl UnixListener {
            pub fn bind(_path: impl AsRef<Path>) -> std_io::Result<Self> {
                Err(std_io::Error::new(
                    std_io::ErrorKind::Unsupported,
                    "unix sockets are unavailable on wasm",
                ))
            }

            pub async fn accept(&self) -> std_io::Result<(UnixStream, PathBuf)> {
                Err(std_io::Error::new(
                    std_io::ErrorKind::Unsupported,
                    "unix sockets are unavailable on wasm",
                ))
            }
        }

        impl UnixStream {
            pub fn connect(_path: impl AsRef<Path>) -> std_io::Result<Self> {
                Err(std_io::Error::new(
                    std_io::ErrorKind::Unsupported,
                    "unix sockets are unavailable on wasm",
                ))
            }

            pub async fn connect_async(_path: impl AsRef<Path>) -> std_io::Result<Self> {
                Self::connect(_path)
            }
        }

        impl io::AsyncRead for UnixStream {
            fn poll_read(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
                _buffer: &mut [u8],
            ) -> Poll<std_io::Result<usize>> {
                let _ = self;
                Poll::Ready(Err(std_io::Error::new(
                    std_io::ErrorKind::Unsupported,
                    "unix sockets are unavailable on wasm",
                )))
            }
        }

        impl io::AsyncWrite for UnixStream {
            fn poll_write(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
                _buffer: &[u8],
            ) -> Poll<std_io::Result<usize>> {
                let _ = self;
                Poll::Ready(Err(std_io::Error::new(
                    std_io::ErrorKind::Unsupported,
                    "unix sockets are unavailable on wasm",
                )))
            }

            fn poll_flush(
                self: Pin<&mut Self>,
                _cx: &mut Context<'_>,
            ) -> Poll<std_io::Result<()>> {
                let _ = self;
                Poll::Ready(Ok(()))
            }

            fn poll_close(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<std_io::Result<()>> {
                self.poll_flush(cx)
            }
        }
    }
}
