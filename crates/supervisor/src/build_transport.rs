use std::io;

use crate::build_protocol::ControlEndpoint;

#[cfg(windows)]
pub type LocalServerStream = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(windows)]
pub type LocalClientStream = tokio::net::windows::named_pipe::NamedPipeClient;

#[cfg(unix)]
pub type LocalServerStream = tokio::net::UnixStream;
#[cfg(unix)]
pub type LocalClientStream = tokio::net::UnixStream;

#[cfg(windows)]
pub struct LocalControlListener {
    endpoint: String,
    first: bool,
    pending: Option<LocalServerStream>,
}

#[cfg(windows)]
impl LocalControlListener {
    pub fn bind(endpoint: &ControlEndpoint) -> io::Result<Self> {
        Ok(Self {
            endpoint: endpoint.display().to_owned(),
            first: true,
            pending: None,
        })
    }

    pub async fn accept(&mut self) -> io::Result<LocalServerStream> {
        use tokio::net::windows::named_pipe::ServerOptions;

        if self.pending.is_none() {
            let mut options = ServerOptions::new();
            options.first_pipe_instance(self.first);
            self.pending = Some(options.create(&self.endpoint)?);
            self.first = false;
        }
        let server = self.pending.as_mut().ok_or_else(|| {
            io::Error::other("named-pipe listener lost its pending server instance")
        })?;
        server.connect().await?;
        self.pending
            .take()
            .ok_or_else(|| io::Error::other("named-pipe listener lost its connected instance"))
    }
}

#[cfg(windows)]
pub async fn connect_local_control(endpoint: &ControlEndpoint) -> io::Result<LocalClientStream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    use tokio::time::{Duration, Instant, sleep};

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match ClientOptions::new().open(endpoint.display()) {
            Ok(client) => return Ok(client),
            Err(error)
                if Instant::now() < deadline
                    && matches!(error.raw_os_error(), Some(code) if code == 2 || code == 231) =>
            {
                sleep(Duration::from_millis(10)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(unix)]
pub struct LocalControlListener {
    listener: tokio::net::UnixListener,
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl LocalControlListener {
    pub fn bind(endpoint: &ControlEndpoint) -> io::Result<Self> {
        use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};

        let path = std::path::PathBuf::from(endpoint.display());
        if path.exists() {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_socket() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("control endpoint is not a socket: {}", path.display()),
                ));
            }
            std::fs::remove_file(&path)?;
        }
        let listener = tokio::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { listener, path })
    }

    pub async fn accept(&mut self) -> io::Result<LocalServerStream> {
        self.listener
            .accept()
            .await
            .map(|(stream, _address)| stream)
    }
}

#[cfg(unix)]
impl Drop for LocalControlListener {
    fn drop(&mut self) {
        let _removed = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
pub async fn connect_local_control(endpoint: &ControlEndpoint) -> io::Result<LocalClientStream> {
    tokio::net::UnixStream::connect(endpoint.display()).await
}
