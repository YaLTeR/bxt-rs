//! Remote connection to the HLTAS Studio.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, TryLockError};
use std::time::{Duration, Instant};
use std::{fmt, thread};

use bxt_ipc_types::Frame;
use color_eyre::eyre::{self, eyre, Context};
use hltas::HLTAS;
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::hooks::engine::RngState;
use crate::hooks::{bxt, engine};
use crate::modules::remote_forbid;
use crate::utils::{MainThreadCell, MainThreadMarker, PointerTrait};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayRequest {
    /// Script to play.
    pub script: HLTAS,
    /// Generation of the project being played.
    pub generation: u16,
    /// Index of the branch being played.
    pub branch_idx: usize,
    /// Whether we're playing through the smoothed version of the script.
    pub is_smoothed: bool,
}

/// Data for accurate frame response.
#[derive(Clone, Serialize, Deserialize)]
pub struct AccurateFrame {
    /// Index of this frame.
    pub frame_idx: usize,
    /// Data of this frame.
    pub frame: Frame,
    /// Generation of the project being played.
    pub generation: u16,
    /// Index of the branch being played.
    pub branch_idx: usize,
    /// Whether we're playing through the smoothed version of the script.
    pub is_smoothed: bool,
    pub random_seed: u32,
    pub rng_state: RngState,
}

impl fmt::Debug for AccurateFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccurateFrame")
            .field("pos", &self.frame.state.player.pos)
            .field("generation", &self.generation)
            .finish()
    }
}

enum State {
    Server(Option<RemoteClient>),
    Client {
        sender: IpcSender<AccurateFrame>,
        receiver: IpcReceiver<PlayRequest>,
    },
}

impl State {
    /// Returns `true` if the state is [`Client`].
    ///
    /// [`Client`]: State::Client
    #[must_use]
    fn is_client(&self) -> bool {
        matches!(self, Self::Client { .. })
    }
}

struct RemoteClient {
    receiver: IpcReceiver<AccurateFrame>,
    // The sender is split into a separate Mutex.
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static REMOTE_CLIENT_SENDER: Mutex<Option<IpcSender<PlayRequest>>> = Mutex::new(None);

/// Whether the client connection thread should try connecting to the remote server.
static SHOULD_CONNECT_TO_SERVER: AtomicBool = AtomicBool::new(false);

static STARTED_CLIENT_CONNECTION_THREAD: MainThreadCell<bool> = MainThreadCell::new(false);

/// The port that we use for communication between the server and the clients.
static PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("BXT_RS_STUDIO_REMOTE_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        // One of the unassigned ports according to
        // https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.txt.
        .unwrap_or(42402)
});

#[instrument(name = "remote::start_server", skip_all)]
pub fn start_server() -> eyre::Result<()> {
    let mut state = STATE.lock().unwrap();

    match *state {
        None => {}
        Some(State::Client { .. }) => return Err(eyre!("already connected to a remote server")),
        Some(State::Server(_)) => return Ok(()),
    }

    let listener =
        TcpListener::bind(("127.0.0.1", *PORT)).context("error binding the TcpListener")?;

    *state = Some(State::Server(None));
    drop(state);

    thread::Builder::new()
        .name("HLTAS Studio Server Thread".to_string())
        .spawn(move || server_thread(listener))
        .unwrap();

    Ok(())
}

fn server_thread(listener: TcpListener) {
    for stream in listener.incoming() {
        let _span = info_span!("accepting remote client connection").entered();

        let mut state = STATE.lock().unwrap();
        if let Some(State::Server(Some(_))) = *state {
            trace!("continuing because already have a client");
            continue;
        }

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

        let (_, (request_sender, workaround_sender)): (_, (_, IpcSender<_>)) = match server.accept()
        {
            Ok(x) => x,
            Err(err) => {
                error!("Error accepting remote client IPC connection: {err:?}");
                continue;
            }
        };

        let (frames_sender, frames_receiver) = match ipc_channel::ipc::channel() {
            Ok(x) => x,
            Err(err) => {
                error!("Error creating a frames IPC channel: {err:?}");
                return;
            }
        };

        if let Err(err) = workaround_sender.send(frames_sender) {
            error!("Error sending the frames sender to the remote client: {err:?}");
            return;
        };

        let mut sender = REMOTE_CLIENT_SENDER.lock().unwrap();
        *state = Some(State::Server(Some(RemoteClient {
            receiver: frames_receiver,
        })));
        *sender = Some(request_sender);
    }
}

/// Starts a thread that tries to connect to a remote server repeatedly.
#[instrument(
    name = "tas_studio::remote::maybe_start_client_connection_thread",
    skip_all
)]
pub fn maybe_start_client_connection_thread(marker: MainThreadMarker) {
    if STARTED_CLIENT_CONNECTION_THREAD.get(marker) {
        return;
    }

    if !engine::Host_FilterTime.is_set(marker) {
        // We will never try to receive the script.
        return;
    }

    if !bxt::BXT_TAS_LOAD_SCRIPT_FROM_STRING.is_set(marker) {
        // We won't be able to run the scripts.
        return;
    }

    if bxt::is_simulation_ipc_client(marker) {
        // We are a BXT simulation client.
        return;
    }

    SHOULD_CONNECT_TO_SERVER.store(true, std::sync::atomic::Ordering::SeqCst);

    STARTED_CLIENT_CONNECTION_THREAD.set(marker, true);

    thread::Builder::new()
        .name("HLTAS Studio Client Connection Thread".to_string())
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

        let state = STATE.lock().unwrap();
        if state.is_some() {
            // Already connected or server, keep looping.
            continue;
        }
        // Windows TcpStream::connect() blocks.
        drop(state);

        let stream = match TcpStream::connect(("127.0.0.1", *PORT)) {
            Ok(x) => x,
            Err(err) => {
                // Don't print an error if the server does not exist yet.
                if err.kind() != std::io::ErrorKind::ConnectionRefused {
                    error!("error connecting to server: {err:?}");
                }

                continue;
            }
        };

        let (receiver, sender) = match connect_to_server(stream) {
            Ok(x) => x,
            Err(err) => {
                // The server will refuse connections if it doesn't need one. Use trace!() and no
                // backtrace (no :?) to not spam the log.
                trace!("error connecting to server: {err}");
                continue;
            }
        };

        let mut state = STATE.lock().unwrap();
        if state.is_some() {
            // Already connected or a server.
            continue;
        }
        *state = Some(State::Client { sender, receiver });
    }
}

fn connect_to_server(
    mut stream: TcpStream,
) -> eyre::Result<(IpcReceiver<PlayRequest>, IpcSender<AccurateFrame>)> {
    // The first messages are trace!() because they will spam every second if we're trying to
    // connect to a server which already has a game connected.
    trace!("reading IPC name from server");

    let mut name = String::new();
    stream
        .read_to_string(&mut name)
        .context("error reading IPC name from server")?;
    drop(stream);

    trace!("connecting to server IPC");
    let tx = IpcSender::connect(name).context("error connecting to the server IPC")?;

    let (hltas_sender, request_receiver) =
        ipc_channel::ipc::channel().context("error creating HLTAS IPC channel")?;

    // Workaround for a Windows ipc-channel panic: the receiver for large payloads should be created
    // in the process that will be using it.
    //
    // https://github.com/servo/ipc-channel/issues/277
    let (workaround_sender, workaround_receiver) =
        ipc_channel::ipc::channel().context("error creating workaround IPC channel")?;

    trace!("sending senders to server");
    tx.send((hltas_sender, workaround_sender))
        .context("error sending IPC channels to server")?;

    trace!("receiving sender from server");
    let response_sender = workaround_receiver
        .recv()
        .context("error receiving frames sender from server")?;

    debug!("connected to remote server");

    Ok((request_receiver, response_sender))
}

pub fn update_client_connection_condition(marker: MainThreadMarker) {
    if remote_forbid::should_forbid(marker) || bxt::is_simulation_ipc_client(marker) {
        SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);

        // Disconnect if we were connected.
        let mut state = STATE.lock().unwrap();
        let mut sender = REMOTE_CLIENT_SENDER.lock().unwrap();
        *state = None;
        *sender = None;

        return;
    }

    match STATE.try_lock() {
        Err(TryLockError::Poisoned(guard)) => panic!("{guard:?}"),
        Ok(state) => {
            if state.is_some() {
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

pub fn is_connected_to_server() -> bool {
    STATE.try_lock().map_or(false, |state| {
        state.as_ref().map_or(false, |state| state.is_client())
    })
}

pub fn receive_request_from_server() -> Result<Option<PlayRequest>, ()> {
    let mut state = match STATE.try_lock() {
        Err(TryLockError::Poisoned(guard)) => panic!("{guard:?}"),
        Err(TryLockError::WouldBlock) => return Ok(None),
        Ok(state) => state,
    };
    let Some(State::Client { receiver, .. }) = state.as_mut() else {
        return Ok(None);
    };

    match receiver.try_recv() {
        Ok(request) => Ok(Some(request)),
        Err(ipc_channel::ipc::TryRecvError::Empty) => Ok(None),
        Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => {
            // TODO: propagate error, print outside.
            error!("error receiving request from server: {err:?}");
            *state = None;
            Err(())
        }
    }
}

#[instrument(skip_all)]
pub fn send_frame_to_server(frame: AccurateFrame) -> Result<(), ()> {
    let mut state = STATE.lock().unwrap();
    let Some(State::Client { sender, .. }) = state.as_mut() else {
        return Err(());
    };

    match sender.send(frame) {
        Ok(()) => Ok(()),
        Err(err) => {
            // TODO: propagate error, print outside.
            error!("error sending frame to server: {err:?}");
            *state = None;
            Err(())
        }
    }
}

pub fn receive_frame_from_client() -> Result<Option<AccurateFrame>, ()> {
    let mut state = match STATE.try_lock() {
        Err(TryLockError::Poisoned(guard)) => panic!("{guard:?}"),
        Err(TryLockError::WouldBlock) => return Ok(None),
        Ok(state) => state,
    };
    let Some(State::Server(Some(RemoteClient { receiver, .. }))) = state.as_mut() else {
        return Ok(None);
    };

    match receiver.try_recv() {
        Ok(frame) => Ok(Some(frame)),
        Err(ipc_channel::ipc::TryRecvError::Empty) => Ok(None),
        Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => {
            // TODO: propagate error, print outside.
            error!("error receiving frame from client: {err:?}");
            *state = Some(State::Server(None));
            Err(())
        }
    }
}

#[instrument(skip_all)]
pub fn maybe_send_request_to_client(request: PlayRequest) {
    // Spawn a thread to send the request. This way we avoid the deadlock where we try to send a new
    // request while the client tries to send us new accurate frames.
    thread::Builder::new()
        .name("HLTAS Studio Send Request to Client Thread".to_owned())
        .spawn(move || {
            let mut remote_sender = REMOTE_CLIENT_SENDER.lock().unwrap();
            let Some(sender) = remote_sender.as_mut() else {
                return;
            };

            if let Err(err) = sender.send(request) {
                // TODO: propagate error, print outside.
                error!("error sending request to client: {err:?}");
                let mut state = STATE.lock().unwrap();
                *state = Some(State::Server(None));
                *remote_sender = None;
            }
        })
        .unwrap();
}
