//! Remote game script execution support.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{mem, thread};

use color_eyre::eyre::{self, eyre, Context};
use hltas::HLTAS;
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use once_cell::sync::Lazy;

use super::optimizer::Frame;
use crate::hooks::{bxt, engine};
use crate::utils::{MainThreadCell, MainThreadMarker, PointerTrait};

#[derive(Debug, Clone)]
pub enum RemoteGameState {
    Free,
    Busy {
        /// The script the game is simulating.
        hltas: HLTAS,
        /// The generation of the script, incremented every time a new source HLTAS is loaded.
        generation: u16,
    },
}

pub struct RemoteGame {
    sender: IpcSender<HLTAS>,
    receiver: IpcReceiver<Vec<Frame>>,
    state: RemoteGameState,
}

enum SimulationState {
    Idle,
    WaitingToStart,
    Recording(Vec<Frame>),
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::Idle
    }
}

impl SimulationState {
    /// Returns `true` if the simulation state is [`Idle`].
    ///
    /// [`Idle`]: SimulationState::Idle
    #[must_use]
    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Returns `true` if the simulation state is [`WaitingToStart`].
    ///
    /// [`WaitingToStart`]: SimulationState::WaitingToStart
    #[must_use]
    fn is_waiting_to_start(&self) -> bool {
        matches!(self, Self::WaitingToStart)
    }

    fn take_frames(&mut self) -> Option<Vec<Frame>> {
        if let SimulationState::Recording(frames) = mem::take(self) {
            Some(frames)
        } else {
            None
        }
    }
}

struct RemoteServer {
    receiver: IpcReceiver<HLTAS>,
    sender: IpcSender<Vec<Frame>>,
    simulation_state: SimulationState,
}

/// Remote state.
enum State {
    /// We are neither a client nor a server.
    None,
    /// We are a client.
    Client(RemoteServer),
    /// We are a server.
    Server(Vec<RemoteGame>),
}

impl State {
    /// Returns `true` if the state is [`None`].
    ///
    /// [`None`]: State::None
    #[must_use]
    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    fn unwrap_server(&mut self) -> &mut Vec<RemoteGame> {
        match self {
            Self::Server(x) => x,
            _ => panic!("called `State::unwrap_server()` on a non-`Server` value"),
        }
    }

    /// Returns `true` if the state is [`Client`].
    ///
    /// [`Client`]: State::Client
    #[must_use]
    fn is_client(&self) -> bool {
        matches!(self, Self::Client(..))
    }

    fn remote_server(&mut self) -> Option<&mut RemoteServer> {
        if let Self::Client(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

static STATE: Mutex<State> = Mutex::new(State::None);

/// Whether the client connection thread should try connecting to the remote server.
static SHOULD_CONNECT_TO_SERVER: AtomicBool = AtomicBool::new(false);

static STARTED_CLIENT_CONNECTION_THREAD: MainThreadCell<bool> = MainThreadCell::new(false);

/// The port that we use for communication between the server and the clients.
static PORT: Lazy<u16> = Lazy::new(|| {
    std::env::var("BXT_RS_REMOTE_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        // One of the unassigned ports according to
        // https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.txt.
        .unwrap_or(42401)
});

impl RemoteGame {
    pub fn is_free(&self) -> bool {
        matches!(self.state, RemoteGameState::Free)
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.state, RemoteGameState::Busy { .. })
    }

    pub fn busy_generation(&self) -> Option<u16> {
        if let RemoteGameState::Busy { generation, .. } = self.state {
            Some(generation)
        } else {
            None
        }
    }

    pub fn unwrap_busy_hltas(&mut self) -> (HLTAS, u16) {
        match mem::replace(&mut self.state, RemoteGameState::Free) {
            RemoteGameState::Free => panic!(),
            RemoteGameState::Busy { hltas, generation } => (hltas, generation),
        }
    }

    pub fn start_simulating(
        &mut self,
        hltas: HLTAS,
        generation: u16,
    ) -> Result<(), (Box<ipc_channel::ErrorKind>, HLTAS)> {
        assert!(self.is_free());

        match self.sender.send(hltas.clone()) {
            Ok(()) => {
                self.state = RemoteGameState::Busy { hltas, generation };
                Ok(())
            }
            Err(err) => Err((err, hltas)),
        }
    }

    pub fn try_recv_frames(
        &mut self,
    ) -> Result<Option<(HLTAS, u16, Vec<Frame>)>, ipc_channel::ipc::IpcError> {
        assert!(self.is_busy());

        match self.receiver.try_recv() {
            Ok(x) => {
                let (hltas, generation) = self.unwrap_busy_hltas();
                Ok(Some((hltas, generation, x)))
            }
            Err(ipc_channel::ipc::TryRecvError::Empty) => Ok(None),
            Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => Err(err),
        }
    }
}

#[instrument(name = "remote::start_server", skip_all)]
pub fn start_server() -> eyre::Result<()> {
    let mut state = STATE.lock().unwrap();

    match *state {
        State::None => {}
        State::Client(_) => return Err(eyre!("already connected to a remote server")),
        State::Server(_) => return Ok(()),
    }

    let listener =
        TcpListener::bind(("127.0.0.1", *PORT)).context("error binding the TcpListener")?;

    *state = State::Server(Vec::new());
    drop(state);

    thread::Builder::new()
        .name("TAS Optimizer Server Thread".to_string())
        .spawn(move || server_thread(listener))
        .unwrap();

    Ok(())
}

fn server_thread(listener: TcpListener) {
    for stream in listener.incoming() {
        let _span = info_span!("accepting remote client connection").entered();

        let mut stream = match stream {
            Ok(x) => x,
            Err(err) => {
                error!("Error accepting remote client connection: {err:?}");
                continue;
            }
        };

        let (server, name) = IpcOneShotServer::new().unwrap();

        info!("Accepted remote client connection, replying with name: {name}");

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
                error!("Error creating a frames IPC channel: {err:?}");
                return;
            }
        };

        if let Err(err) = workaround_sender.send(frames_sender) {
            error!("Error sending the frames sender to the remote client: {err:?}");
            return;
        };

        STATE.lock().unwrap().unwrap_server().push(RemoteGame {
            sender: hltas_sender,
            receiver: frames_receiver,
            state: RemoteGameState::Free,
        });
    }
}

/// Starts a thread that tries to connect to a remote server repeatedly.
#[instrument(name = "remote::maybe_start_client_connection_thread", skip_all)]
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
        .name("TAS Optimizer Client Connection Thread".to_string())
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

        let mut state = STATE.lock().unwrap();
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
        ipc_channel::ipc::channel().context("error creating a HLTAS IPC channel")?;

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
        .context("error receiving the frames sender from the remote server")?;

    Ok(RemoteServer {
        receiver: hltas_receiver,
        sender: frames_sender,
        simulation_state: SimulationState::Idle,
    })
}

pub fn update_client_connection_condition(marker: MainThreadMarker) {
    if bxt::is_simulation_ipc_client(marker) {
        // Don't try to connect if we're the BXT IPC client.
        SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);

        // Disconnect if we were connected.
        let mut state = STATE.lock().unwrap();
        if state.is_client() {
            *state = State::None;
        }

        return;
    }

    if !STATE.lock().unwrap().is_none() {
        // Don't try to connect if we're already a client or a server.
        SHOULD_CONNECT_TO_SERVER.store(false, std::sync::atomic::Ordering::SeqCst);
        return;
    }

    // Otherwise, try to connect again.
    SHOULD_CONNECT_TO_SERVER.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn is_connected_to_server() -> bool {
    STATE.lock().unwrap().is_client()
}

/// Receives any completed simulation results from the remote clients and calls `process_result` to
/// process them.
///
/// Note that the returned frames can contain less or more frames than there are in the HLTAS due to
/// currently inaccurate frame recording.
pub fn receive_simulation_result_from_clients(
    mut process_result: impl FnMut(HLTAS, u16, Vec<Frame>),
) {
    let mut state = STATE.lock().unwrap();
    let games = state.unwrap_server();

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_busy()) {
        match game.try_recv_frames() {
            Ok(Some((hltas, generation, frames))) => process_result(hltas, generation, frames),
            Ok(None) => (),
            Err(err) => {
                error!("Error receiving simulation result from a remote client: {err:?}");
                errored_indices.push(index);
            }
        }
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }
}

/// Returns `true` if any of the remote clients is currently simulating this generation.
pub fn is_any_client_simulating_generation(generation: u16) -> bool {
    STATE
        .lock()
        .unwrap()
        .unwrap_server()
        .iter()
        .any(|g| g.busy_generation() == Some(generation))
}

/// For all available (non-busy) remote clients, calls `prepare_hltas` to prepare a HLTAS and sends
/// it to the remote client for simulation.
///
/// If an error occurs sending the HLTAS to the remote client, it will be silently dropped without
/// simulating.
pub fn simulate_in_available_clients(mut prepare_hltas: impl FnMut() -> (HLTAS, u16)) {
    let mut state = STATE.lock().unwrap();
    let games = state.unwrap_server();

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_free()) {
        let (hltas, generation) = prepare_hltas();

        if let Err((err, _)) = game.start_simulating(hltas, generation) {
            error!("Error sending HLTAS to a remote client: {err:?}");
            errored_indices.push(index);
        }
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }
}

/// Finds one available (non-busy) remote client, calls `prepare_hltas` to prepare a HLTAS and sends
/// it for simulation. If the client errors out, finds the next one and sends the HLTAS there, and
/// so on.
///
/// If there was no success in finding a free client and sending it the HLTAS, it is silently
/// dropped without simulating.
pub fn maybe_simulate_in_one_client(mut prepare_hltas: impl FnMut() -> (HLTAS, u16)) {
    let mut state = STATE.lock().unwrap();
    let games = state.unwrap_server();

    let mut payload = None;

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_free()) {
        let (hltas, generation) = payload.take().unwrap_or_else(&mut prepare_hltas);

        if let Err((err, hltas)) = game.start_simulating(hltas, generation) {
            error!("Error sending HLTAS to a remote client: {err:?}");
            errored_indices.push(index);
            payload = Some((hltas, generation));
            continue;
        }

        break;
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }
}

pub fn receive_new_hltas_to_simulate() -> Option<HLTAS> {
    let mut state = STATE.lock().unwrap();
    let server = state.remote_server()?;

    if !server.simulation_state.is_idle() {
        // Already simulating something.
        return None;
    }

    match server.receiver.try_recv() {
        Ok(hltas) => {
            server.simulation_state = SimulationState::WaitingToStart;
            return Some(hltas);
        }
        Err(ipc_channel::ipc::TryRecvError::Empty) => (),
        Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => {
            error!("Error when receiving a HLTAS from the remote server: {err:?}");
            *state = State::None;
        }
    }

    None
}

pub fn on_frame_simulated(get_frame_data: impl FnOnce() -> Frame) {
    let mut state = STATE.lock().unwrap();
    let Some(server) = state.remote_server() else { return };

    let SimulationState::Recording(pending_frames) = &mut server.simulation_state else { return };

    let frame = get_frame_data();

    pending_frames.push(frame);
}

pub fn send_simulation_result_to_server() {
    let mut state = STATE.lock().unwrap();
    let Some(server) = state.remote_server() else { return };

    // Send empty frames if needed to avoid softlocks in unforeseen situations.
    let pending_frames = server.simulation_state.take_frames().unwrap_or_default();

    if let Err(err) = server.sender.send(pending_frames) {
        error!("Error when trying to send frames to the server: {err:?}");
        *state = State::None;
    }
}

pub fn start_recording_frames() {
    let mut state = STATE.lock().unwrap();
    let Some(server) = state.remote_server() else { return };

    if server.simulation_state.is_waiting_to_start() {
        server.simulation_state = SimulationState::Recording(Vec::new());
    }
}
