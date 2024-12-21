//! Enables multi-game capture for bxt_cap_separate

use std::ffi::CStr;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::str::from_utf8;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

use color_eyre::eyre::{self, eyre, Context};
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use super::{CaptureVideoPerDemo, BXT_CAP_SEPARATE_MULTIGAME_EXEC, TARGET_DIR};
use crate::hooks::engine;
use crate::modules::{demo_playback, remote_forbid, Module};
use crate::utils::*;

struct RemoteRecorder {
    sender: IpcSender<RecordRequest>,
    receiver: IpcReceiver<IsFree>,
    is_free: IsFree,
}

struct RemoteServer {
    sender: IpcSender<IsFree>,
    receiver: IpcReceiver<RecordRequest>,
}

type IsFree = bool;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordRequest {
    /// Path to demo (.dem)
    pub demo_path: String,
    /// Path to output video (.mp4)
    pub output_path: String,
    /// Path to config file (.cfg) before recording
    pub exec_path: String,
}

enum State {
    None,
    Server(Vec<RemoteRecorder>),
    Client(RemoteServer),
}

impl State {
    /// Returns `true` if the state is [`Client`].
    ///
    /// [`Client`]: State::Client
    #[must_use]
    fn is_client(&self) -> bool {
        matches!(self, Self::Client { .. })
    }

    fn unwrap_server(&mut self) -> &mut Vec<RemoteRecorder> {
        match self {
            Self::Server(x) => x,
            _ => panic!("called `State::unwrap_server()` on a non-`Server` value"),
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

// IPC main stuffs
pub unsafe fn maybe_receive_request_from_remote_server(marker: MainThreadMarker) {
    let Some(cls) = engine::cls.get_opt(marker) else {
        return;
    };

    // do not record while in server
    let client_state = (*cls).state;
    if client_state != 1 {
        return;
    }

    if !STATE.lock().unwrap().is_client() {
        return;
    }

    // let server knows that it is free
    maybe_send_free_status_to_server();

    if let Some(request) = receive_request_from_server() {
        engine::prepend_command(
            marker,
            format!(
                "exec {};playdemo {};bxt_cap_start {};\n",
                request.exec_path, request.demo_path, request.output_path
            )
            .as_str(),
        );
    }
}

fn receive_request_from_server() -> Option<RecordRequest> {
    let mut state = match STATE.try_lock() {
        Err(TryLockError::Poisoned(guard)) => panic!("{guard:?}"),
        Err(TryLockError::WouldBlock) => return None,
        Ok(state) => state,
    };

    let State::Client(RemoteServer { ref receiver, .. }) = *state else {
        return None;
    };

    match receiver.try_recv() {
        Ok(request) => Some(request),
        Err(ipc_channel::ipc::TryRecvError::Empty) => None,
        Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => {
            // TODO: propagate error, print outside.
            error!("error receiving request from server: {err:?}");
            *state = State::None;
            None
        }
    }
}

pub fn maybe_send_free_status_to_server() {
    let mut state = STATE.lock().unwrap();
    let State::Client(ref server) = *state else {
        return;
    };

    if let Err(err) = server.sender.send(true) {
        error!("Error when trying to send free status to the server: {err:?}");
        *state = State::None;
    }
}

pub fn maybe_receive_status_and_send_requests(marker: MainThreadMarker) {
    // thread::Builder::new()
    //     .name("Receive request from client thread".to_owned())
    //     .spawn(move || {
    let mut state = STATE.lock().unwrap();

    if !matches!(*state, State::Server(..)) {
        return;
    }

    let games = state.unwrap_server();
    let mut errored_indices = Vec::new();

    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| !g.is_free) {
        let res = match game.receiver.try_recv() {
            Ok(x) => Ok(Some(x)),
            Err(ipc_channel::ipc::TryRecvError::Empty) => Ok(None),
            Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => Err(err),
        };

        // res is always `true`
        match res {
            Ok(Some(res)) => game.is_free = res,
            Ok(None) => (),
            Err(err) => {
                error!("Error receiving simulation result from a remote client: {err:?}");
                errored_indices.push(index);
            }
        }
    }

    for index in errored_indices.iter().rev() {
        games.remove(*index);
    }

    errored_indices.clear();

    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_free) {
        let mut demos = demo_playback::DEMOS.borrow_mut(marker);

        if index >= demos.len() {
            break;
        }

        let demo_path_bytes = demos.remove(index);
        // to remove null
        let demo_path = CStr::from_bytes_with_nul(&demo_path_bytes)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let output_path = if let Some(ref path) = *TARGET_DIR.borrow(marker) {
            path.join(demo_path.as_str())
                .with_extension("mp4")
                .display()
                .to_string()
        } else {
            let game_dir = unsafe { &*engine::com_gamedir.get(marker) };
            let game_dir = game_dir.iter().map(|&i| i as u8).collect::<Vec<u8>>();

            // jesus
            let game_dir = from_utf8(game_dir.as_slice()).unwrap().replace("\0", "");

            let output_file = PathBuf::from(demo_path.as_str()).with_extension("mp4");

            PathBuf::from(game_dir)
                .join(output_file)
                .display()
                .to_string()
        };

        // better be safe
        let exec_path = BXT_CAP_SEPARATE_MULTIGAME_EXEC
            .to_string(marker)
            .replace("\0", "");

        let request = RecordRequest {
            demo_path,
            output_path,
            exec_path,
        };

        match game.sender.send(request) {
            Ok(()) => {
                game.is_free = false;
            }
            Err(err) => {
                error!("Error sending recording request to a remote client: {err:?}");
                errored_indices.push(index);

                // if errors, which always happens when the server first check all the clients
                // get the demos back to the queue
                // it seems like the fix for that first time thing is pretty weird
                // because it won't look like the original ipc code
                // and i don't want to come up with the fix

                demos.push(demo_path_bytes);
            }
        }
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }

    // })
    // .unwrap();
}

// IPC setup stuffs

static STATE: Mutex<State> = Mutex::new(State::None);

static REMOTE_CLIENT_SENDER: Mutex<Option<IpcSender<RecordRequest>>> = Mutex::new(None);
static SHOULD_CONNECT_TO_SERVER: AtomicBool = AtomicBool::new(false);

static STARTED_CLIENT_CONNECTION_THREAD: MainThreadCell<bool> = MainThreadCell::new(false);

/// The port that we use for communication between the server and the clients.
static PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("BXT_RS_CAP_SEPARATE_MULTIGAME")
        .ok()
        .and_then(|value| value.parse().ok())
        // One of the unassigned ports according to
        // https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.txt.
        .unwrap_or(42403)
});

#[instrument(name = "cap_separate_multigame_remote::start_server", skip_all)]
pub fn start_server() -> eyre::Result<()> {
    let mut state = STATE.lock().unwrap();

    match *state {
        State::None => {}
        State::Client { .. } => return Err(eyre!("already connected to a remote server")),
        State::Server(_) => return Ok(()),
    }

    let listener =
        TcpListener::bind(("127.0.0.1", *PORT)).context("error binding the TcpListener")?;

    *state = State::Server(Vec::new());
    drop(state);

    thread::Builder::new()
        .name("bxt_cap_separate_multigame Server Thread".to_string())
        .spawn(move || server_thread(listener))
        .unwrap();

    Ok(())
}

fn server_thread(listener: TcpListener) {
    for stream in listener.incoming() {
        let _span = info_span!("accepting remote client connection").entered();

        // let mut state = STATE.lock().unwrap();
        // if let Some(State::Server(Some(_))) = *state {
        //     trace!("continuing because already have a client");
        //     continue;
        // }

        let mut stream = match stream {
            Ok(x) => x,
            Err(err) => {
                error!("Error accepting remote client connection: {err:?}");
                continue;
            }
        };

        let (server, name) = IpcOneShotServer::new().unwrap();

        debug!("Accepted remote client connection, replying with name: {name}");

        if let Err(err) = stream.write_all(name.as_bytes()) {
            error!("Error sending IPC server name to the remote client: {err:?}");
            continue;
        }
        drop(stream);

        let (_, (hltas_sender, workaround_sender)): (_, (_, IpcSender<_>)) = match server.accept() {
            Ok(x) => x,
            Err(err) => {
                error!("Error accepting remote client IPC connection: {err:?}");
                continue;
            }
        };

        let (frames_sender, frames_receiver) = match ipc_channel::ipc::channel() {
            Ok(x) => x,
            Err(err) => {
                error!("Error creating an IPC channel: {err:?}");
                return;
            }
        };

        if let Err(err) = workaround_sender.send(frames_sender) {
            error!("Error sending the sender to the remote client: {err:?}");
            return;
        };

        STATE.lock().unwrap().unwrap_server().push(RemoteRecorder {
            sender: hltas_sender,
            receiver: frames_receiver,
            is_free: true,
        });
    }
}

/// Starts a thread that tries to connect to a remote server repeatedly.
#[instrument(
    name = "cap_separate_multigame_remote::maybe_start_client_connection_thread",
    skip_all
)]
pub fn maybe_start_client_connection_thread(marker: MainThreadMarker) {
    if !CaptureVideoPerDemo.is_enabled(marker) {
        return;
    }

    if STARTED_CLIENT_CONNECTION_THREAD.get(marker) {
        return;
    }

    SHOULD_CONNECT_TO_SERVER.store(true, std::sync::atomic::Ordering::SeqCst);

    STARTED_CLIENT_CONNECTION_THREAD.set(marker, true);

    thread::Builder::new()
        .name("bxt_cap_separate_multigame Client Connection Thread".to_string())
        .spawn(client_connection_thread)
        .unwrap();
}

fn client_connection_thread() {
    let mut last_attempted_at = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);

    loop {
        thread::sleep(Duration::from_secs(1).saturating_sub(last_attempted_at.elapsed()));
        last_attempted_at = Instant::now();

        if !SHOULD_CONNECT_TO_SERVER.load(std::sync::atomic::Ordering::SeqCst) {
            continue;
        }

        let mut state = STATE.lock().unwrap();

        // if matches!(*state, State::Server(..)) {
        //     // ouroboros, which will always happen, this line is to prevent it
        //     SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);
        //     continue;
        // }

        let stream = match TcpStream::connect(("127.0.0.1", *PORT)) {
            Ok(x) => x,
            Err(err) => {
                // Don't print an error if the server does not exist yet.
                if err.kind() != std::io::ErrorKind::ConnectionRefused {
                    error!("Error connecting to the remote server: {err:?}");
                }

                continue;
            }
        };

        let server = match connect_to_server(stream) {
            Ok(x) => x,
            Err(err) => {
                error!("Error connecting to the remote server: {err:?}");
                continue;
            }
        };

        info!("Connected to a remote server.");

        if state.is_none() {
            *state = State::Client(server);
        } else {
            // The check is only done here and not before connect_to_server() because we must go
            // through with the IPC connection, otherwise the server will block indefinitely while
            // waiting for the client to connect.
            info!("Dropping a successful remote server connection because the state is not None.");
        }
    }
}

fn connect_to_server(mut stream: TcpStream) -> eyre::Result<RemoteServer> {
    let mut name = String::new();
    stream
        .read_to_string(&mut name)
        .context("error reading IPC name from the remote server")?;
    drop(stream);

    let tx = IpcSender::connect(name).context("error connecting to the remote server IPC")?;

    let (hltas_sender, hltas_receiver) =
        ipc_channel::ipc::channel().context("error creating an IPC channel")?;

    // Workaround for a Windows ipc-channel panic: the receiver for large payloads should be created
    // in the process that will be using it.
    //
    // https://github.com/servo/ipc-channel/issues/277
    let (workaround_sender, workaround_receiver) =
        ipc_channel::ipc::channel().context("error creating a workaround IPC channel")?;

    tx.send((hltas_sender, workaround_sender))
        .context("error sending the IPC channels to the remote server")?;

    let frames_sender = workaround_receiver
        .recv()
        .context("error receiving the sender from the remote server")?;

    Ok(RemoteServer {
        receiver: hltas_receiver,
        sender: frames_sender,
    })
}

pub fn update_client_connection_condition(marker: MainThreadMarker) {
    if remote_forbid::should_forbid(marker) {
        SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);

        // Disconnect if we were connected.
        let mut state = STATE.lock().unwrap();
        let mut sender = REMOTE_CLIENT_SENDER.lock().unwrap();
        *state = State::None;
        *sender = None;

        return;
    }

    match STATE.try_lock() {
        Err(TryLockError::Poisoned(guard)) => panic!("{guard:?}"),
        Ok(state) => {
            if !state.is_none() {
                // Don't try to connect if we're already a client or a server.
                SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);
                return;
            }
        }
        // If we failed to check because of a locked mutex, just return for now, and don't set to
        // true below.
        _ => return,
    }

    // Otherwise, try to connect again.
    SHOULD_CONNECT_TO_SERVER.store(true, std::sync::atomic::Ordering::SeqCst);
}
