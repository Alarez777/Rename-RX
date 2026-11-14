//! Single-instance handling for the primary Rename RX window.
//!
//! The first process binds a Unix socket under the user's runtime directory.
//! Later launches connect to it, send their file arguments, and exit. The
//! primary decides whether to add the files directly or ask the user.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const APP_DIRECTORY: &str = "rename-rx";
const SOCKET_FILE: &str = "instance-v1.sock";
/// Message marker so a stale or foreign socket is never mistaken for ours.
const MAGIC: u32 = 0x5252_5801;
/// Linux `sun_path` holds 108 bytes including the terminating NUL.
const MAX_SOCKET_PATH_BYTES: usize = 107;
const MAX_PATHS: usize = 65_536;
const MAX_PATH_BYTES: usize = 16 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
/// Retries while another launch is binding the socket at the same time.
const CONNECT_ATTEMPTS: usize = 5;
const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(20);
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

/// What the startup path should do after checking for another instance.
#[derive(Debug)]
pub(crate) enum Startup {
    /// This process owns the socket and should run the application.
    Primary(InstanceServer),
    /// Another instance accepted the files; this process should exit.
    Forwarded,
    /// No IPC is available; run normally without single-instance handling.
    Standalone,
}

/// Acquire the instance socket or forward the paths to the existing instance.
pub(crate) fn acquire(paths: &[PathBuf]) -> Startup {
    let Some(socket) = socket_path() else {
        log::warn!("could not determine a runtime directory for the instance socket");
        return Startup::Standalone;
    };

    if let Some(parent) = socket.parent()
        && let Err(error) = prepare_directory(parent)
    {
        log::warn!("could not prepare {}: {error}", parent.display());
        return Startup::Standalone;
    }

    for attempt in 0..CONNECT_ATTEMPTS {
        match forward(&socket, paths) {
            Ok(()) => return Startup::Forwarded,
            Err(ForwardError::NoInstance) => {}
            Err(ForwardError::Failed(error)) => {
                log::warn!("could not contact the existing Rename RX instance: {error}");
                return Startup::Standalone;
            }
        }

        match InstanceServer::try_bind(&socket) {
            Ok(server) => return Startup::Primary(server),
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                // Another launch may be binding the socket at the same time.
                if attempt + 1 < CONNECT_ATTEMPTS {
                    thread::sleep(CONNECT_RETRY_DELAY);
                }
            }
            Err(error) => {
                log::warn!("could not listen for other Rename RX launches: {error}");
                return Startup::Standalone;
            }
        }
    }

    // The socket file exists but nobody accepted our connections. Try once
    // more before removing it, so a slow instance is not unlinked.
    match forward(&socket, paths) {
        Ok(()) => return Startup::Forwarded,
        Err(ForwardError::NoInstance) => {}
        Err(ForwardError::Failed(error)) => {
            log::warn!("could not contact the existing Rename RX instance: {error}");
            return Startup::Standalone;
        }
    }

    match InstanceServer::replace_stale(&socket) {
        Ok(server) => Startup::Primary(server),
        Err(error) => {
            log::warn!("could not replace the instance socket: {error}");
            Startup::Standalone
        }
    }
}

/// The primary's listener. Dropping it removes the socket file.
#[derive(Debug)]
pub(crate) struct InstanceServer {
    socket: PathBuf,
    listener: UnixListener,
}

impl InstanceServer {
    /// Start accepting forwarded paths and wake the UI for each message.
    pub(crate) fn start(
        &self,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Receiver<Vec<PathBuf>>> {
        let listener = self.listener.try_clone()?;
        let (sender, receiver) = mpsc::channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        thread::Builder::new()
            .name("rename-rx-instance".to_owned())
            .spawn(move || serve(listener, sender, wake))?;
        Ok(receiver)
    }

    fn try_bind(socket: &Path) -> io::Result<Self> {
        let listener = UnixListener::bind(socket)?;
        if let Err(error) = fs::set_permissions(socket, fs::Permissions::from_mode(0o600)) {
            drop(listener);
            let _ = fs::remove_file(socket);
            return Err(error);
        }

        Ok(Self {
            socket: socket.to_path_buf(),
            listener,
        })
    }

    fn replace_stale(socket: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(socket)?;
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists and is not a socket", socket.display()),
            ));
        }

        fs::remove_file(socket)?;
        Self::try_bind(socket)
    }
}

impl Drop for InstanceServer {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.socket)
            && error.kind() != io::ErrorKind::NotFound
        {
            log::warn!("could not remove {}: {error}", self.socket.display());
        }
    }
}

/// Errors that distinguish "no instance" from a real transport failure.
#[derive(Debug)]
enum ForwardError {
    NoInstance,
    Failed(io::Error),
}

fn forward(socket: &Path, paths: &[PathBuf]) -> Result<(), ForwardError> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(error) if is_missing_instance(&error) => return Err(ForwardError::NoInstance),
        Err(error) => return Err(ForwardError::Failed(error)),
    };

    stream
        .set_write_timeout(Some(WRITE_TIMEOUT))
        .map_err(ForwardError::Failed)?;

    // The primary runs with a different working directory, so relative paths
    // must be resolved by the process that received them.
    let paths: Vec<PathBuf> = paths
        .iter()
        .map(|path| match std::path::absolute(path) {
            Ok(absolute) => absolute,
            Err(error) => {
                log::warn!("could not resolve {}: {error}", path.display());
                path.clone()
            }
        })
        .collect();

    match write_message(&mut stream, &paths) {
        Ok(()) => Ok(()),
        Err(error) if is_missing_instance(&error) => Err(ForwardError::NoInstance),
        Err(error) => Err(ForwardError::Failed(error)),
    }
}

fn is_missing_instance(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
    )
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the listener and sender are owned by the long-lived accept loop"
)]
fn serve(
    listener: UnixListener,
    sender: mpsc::Sender<Vec<PathBuf>>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        match listener.accept() {
            Ok((mut stream, _address)) => {
                let mut reader = match stream.try_clone() {
                    Ok(reader) => reader,
                    Err(error) => {
                        log::warn!("could not clone an instance connection: {error}");
                        handle_connection(&mut stream, &sender, &wake);
                        continue;
                    }
                };
                let thread_sender = sender.clone();
                let thread_wake = Arc::clone(&wake);

                match thread::Builder::new()
                    .name("rename-rx-request".to_owned())
                    .spawn(move || handle_connection(&mut reader, &thread_sender, &thread_wake))
                {
                    Ok(_handle) => {}
                    Err(error) => {
                        log::warn!("could not start an instance request handler: {error}");
                        handle_connection(&mut stream, &sender, &wake);
                    }
                }
            }
            Err(error) => {
                log::warn!("could not accept an instance connection: {error}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn handle_connection(
    stream: &mut UnixStream,
    sender: &mpsc::Sender<Vec<PathBuf>>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) {
    match read_message(stream) {
        Ok(paths) => {
            if sender.send(paths).is_ok() {
                wake();
            }
        }
        Err(error) => log::warn!("could not read an instance request: {error}"),
    }
}

fn socket_path() -> Option<PathBuf> {
    let preferred = preferred_socket_path()?;
    if preferred.as_os_str().as_bytes().len() <= MAX_SOCKET_PATH_BYTES {
        return Some(preferred);
    }

    let uid = current_uid()?;
    log::warn!(
        "instance socket path {} is too long; using a short fallback",
        preferred.display()
    );
    Some(short_socket_path(uid))
}

fn preferred_socket_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(crate::integration::config_home)?;

    Some(base.join(APP_DIRECTORY).join(SOCKET_FILE))
}

/// Short fallback for unusually long runtime or config paths. The fixed
/// `/tmp` location is deliberate: `TMPDIR` may itself be too long.
fn short_socket_path(uid: u32) -> PathBuf {
    Path::new("/tmp")
        .join(format!("rename-rx-{uid}"))
        .join(SOCKET_FILE)
}

fn current_uid() -> Option<u32> {
    fs::metadata("/proc/self")
        .or_else(|_| match std::env::var_os("HOME").map(PathBuf::from) {
            Some(home) => fs::metadata(home),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        })
        .ok()
        .map(|metadata| metadata.uid())
}

fn prepare_directory(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
}

fn write_message(stream: &mut UnixStream, paths: &[PathBuf]) -> io::Result<()> {
    let count = checked_path_count(paths)?;

    stream.write_all(&MAGIC.to_be_bytes())?;
    stream.write_all(&count.to_be_bytes())?;

    for path in paths {
        let bytes = path.as_os_str().as_bytes();
        let length = u32::try_from(bytes.len())
            .map_err(|_| invalid_input("path too long in the instance request"))?;
        stream.write_all(&length.to_be_bytes())?;
        stream.write_all(bytes)?;
    }

    Ok(())
}

/// Validate the whole message before writing anything, so a rejected request
/// never leaves a partial header on the socket.
fn checked_path_count(paths: &[PathBuf]) -> io::Result<u32> {
    if paths.len() > MAX_PATHS {
        return Err(invalid_input("too many paths in the instance request"));
    }

    let mut total = 0usize;
    for path in paths {
        let length = path.as_os_str().as_bytes().len();
        if length > MAX_PATH_BYTES {
            return Err(invalid_input("path too long in the instance request"));
        }

        total = total
            .checked_add(length)
            .ok_or_else(|| invalid_input("instance request too large"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(invalid_input("instance request too large"));
        }
    }

    u32::try_from(paths.len()).map_err(|_| invalid_input("too many paths in the instance request"))
}

fn read_message(stream: &mut UnixStream) -> io::Result<Vec<PathBuf>> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    let magic = read_u32(stream)?;
    if magic != MAGIC {
        return Err(invalid_data("unexpected instance message"));
    }

    let count = read_u32(stream)?;
    let count = usize::try_from(count).map_err(|_| invalid_data("invalid instance message"))?;
    if count > MAX_PATHS {
        return Err(invalid_data("too many paths in the instance request"));
    }

    let mut paths = Vec::with_capacity(count);
    let mut total = 0usize;
    for _ in 0..count {
        let length = read_u32(stream)?;
        let length =
            usize::try_from(length).map_err(|_| invalid_data("invalid instance message"))?;
        if length > MAX_PATH_BYTES {
            return Err(invalid_data("path too long in the instance request"));
        }

        total = total
            .checked_add(length)
            .ok_or_else(|| invalid_data("instance request too large"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(invalid_data("instance request too large"));
        }

        let mut bytes = vec![0u8; length];
        stream.read_exact(&mut bytes)?;
        paths.push(PathBuf::from(OsString::from_vec(bytes)));
    }

    Ok(paths)
}

fn read_u32(stream: &mut UnixStream) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    stream.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_socket(name: &str) -> (PathBuf, PathBuf) {
        let directory =
            std::env::temp_dir().join(format!("rename-rx-instance-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create the temporary directory");
        let socket = directory.join("instance.sock");
        (directory, socket)
    }

    #[test]
    fn paths_round_trip_including_non_utf8_names() {
        let (mut client, mut server) = UnixStream::pair().expect("create a socket pair");
        let paths = vec![
            PathBuf::from("/tmp/with space/ñandú.txt"),
            PathBuf::from(OsString::from_vec(vec![
                b'/', b't', b'm', b'p', b'/', 0xff, 0xfe,
            ])),
        ];

        write_message(&mut client, &paths).expect("write the message");
        let received = read_message(&mut server).expect("read the message");

        assert_eq!(received, paths);
    }

    #[test]
    fn an_empty_message_is_valid() {
        let (mut client, mut server) = UnixStream::pair().expect("create a socket pair");

        write_message(&mut client, &[]).expect("write the message");
        let received = read_message(&mut server).expect("read the message");

        assert!(received.is_empty());
    }

    #[test]
    fn rejects_an_unknown_message_marker() {
        let (mut client, mut server) = UnixStream::pair().expect("create a socket pair");
        client
            .write_all(&0_u32.to_be_bytes())
            .expect("write the marker");

        let error = read_message(&mut server).expect_err("reject the message");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_paths_that_are_too_long() {
        let (mut client, _server) = UnixStream::pair().expect("create a socket pair");
        let path = PathBuf::from("x".repeat(MAX_PATH_BYTES + 1));

        let error = write_message(&mut client, &[path]).expect_err("reject the long path");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn creates_a_private_socket_directory() {
        let (directory, _socket) = temp_socket("directory");
        let nested = directory.join("runtime").join(APP_DIRECTORY);

        prepare_directory(&nested).expect("prepare the directory");
        let mode = fs::metadata(&nested)
            .expect("read the directory metadata")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(mode, 0o700);
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }

    #[test]
    fn the_short_socket_path_fits_the_limit() {
        let path = short_socket_path(1000);

        assert_eq!(path, PathBuf::from("/tmp/rename-rx-1000/instance-v1.sock"));
        assert!(path.as_os_str().as_bytes().len() <= MAX_SOCKET_PATH_BYTES);
    }

    #[test]
    fn a_stalled_client_does_not_block_forwarding() {
        let (directory, socket) = temp_socket("stalled");
        let server = InstanceServer::try_bind(&socket).expect("bind the socket");
        let receiver = server.start(|| {}).expect("start the listener");

        let stalled = UnixStream::connect(&socket).expect("connect the stalled client");
        forward(&socket, &[PathBuf::from("/tmp/after.txt")]).expect("forward after the stall");
        let forwarded = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("receive the paths");

        assert_eq!(forwarded, vec![PathBuf::from("/tmp/after.txt")]);
        drop(stalled);
        drop(server);
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }

    #[test]
    fn forwards_paths_through_the_listener() {
        let (directory, socket) = temp_socket("forward");
        let server = InstanceServer::try_bind(&socket).expect("bind the socket");
        let receiver = server.start(|| {}).expect("start the listener");

        let relative = PathBuf::from("relative.txt");
        let expected = std::path::absolute(&relative).expect("resolve the relative path");
        forward(&socket, &[PathBuf::from("/tmp/a.txt"), relative]).expect("forward the paths");
        let forwarded = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("receive the paths");

        assert_eq!(forwarded, vec![PathBuf::from("/tmp/a.txt"), expected]);

        drop(server);
        assert!(!socket.exists());
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }

    #[test]
    fn reports_a_missing_instance() {
        let (directory, socket) = temp_socket("missing");

        let result = forward(&socket, &[PathBuf::from("/tmp/a.txt")]);

        assert!(matches!(result, Err(ForwardError::NoInstance)));
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }

    #[test]
    fn replaces_a_stale_socket() {
        let (directory, socket) = temp_socket("stale");
        let listener = UnixListener::bind(&socket).expect("bind the stale socket");
        drop(listener);
        assert!(socket.exists());

        let server = InstanceServer::replace_stale(&socket).expect("replace the stale socket");

        assert!(socket.exists());
        drop(server);
        assert!(!socket.exists());
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }

    #[test]
    fn refuses_to_replace_a_regular_file() {
        let (directory, socket) = temp_socket("regular");
        fs::write(&socket, b"not a socket").expect("write the regular file");

        let error = InstanceServer::replace_stale(&socket).expect_err("reject the regular file");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(socket.exists());
        fs::remove_dir_all(directory).expect("remove the temporary directory");
    }
}
