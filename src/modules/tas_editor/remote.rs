//! Remote game script execution support.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use std::{mem, thread};

use color_eyre::eyre::{self, eyre, Context};
use hltas::HLTAS;
use ipc_channel::ipc::{IpcOneShotServer, IpcReceiver, IpcSender};
use parking_lot::{const_mutex, Mutex};

use super::editor::Frame;
use crate::hooks::{bxt, engine};
use crate::utils::{MainThreadCell, MainThreadMarker, PointerTrait};

#[derive(Debug, Clone)]
pub enum RemoteGameState {
    Free,
    Busy(HLTAS),
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

static STATE: Mutex<State> = const_mutex(State::None);

// One of the unassigned ports according to
// https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.txt.
/// The port that we use for communication between the server and the clients.
const PORT: u16 = 42401;

impl RemoteGame {
    pub fn is_free(&self) -> bool {
        matches!(self.state, RemoteGameState::Free)
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.state, RemoteGameState::Busy(_))
    }

    pub fn unwrap_busy_hltas(&mut self) -> HLTAS {
        match mem::replace(&mut self.state, RemoteGameState::Free) {
            RemoteGameState::Free => panic!(),
            RemoteGameState::Busy(hltas) => hltas,
        }
    }

    pub fn start_simulating(&mut self, hltas: HLTAS) -> Result<(), Box<ipc_channel::ErrorKind>> {
        assert!(self.is_free());

        let rv = self.sender.send(hltas.clone());
        if rv.is_ok() {
            self.state = RemoteGameState::Busy(hltas);
        }
        rv
    }

    pub fn try_recv_frames(
        &mut self,
    ) -> Result<Option<(HLTAS, Vec<Frame>)>, ipc_channel::ipc::IpcError> {
        assert!(self.is_busy());

        match self.receiver.try_recv() {
            Ok(x) => Ok(Some((self.unwrap_busy_hltas(), x))),
            Err(ipc_channel::ipc::TryRecvError::Empty) => Ok(None),
            Err(ipc_channel::ipc::TryRecvError::IpcError(err)) => Err(err),
        }
    }

    #[instrument(name = "RemoteGame::recv_frames", skip_all)]
    pub fn recv_frames(&mut self) -> Result<Vec<Frame>, ipc_channel::ipc::IpcError> {
        assert!(self.is_busy());

        self.state = RemoteGameState::Free;
        self.receiver.recv()
    }
}

#[instrument(name = "remote::start_server", skip_all)]
pub fn start_server() -> eyre::Result<()> {
    let mut state = STATE.lock();

    match *state {
        State::None => {}
        State::Client(_) => return Err(eyre!("already connected to a remote server")),
        State::Server(_) => return Ok(()),
    }

    let listener =
        TcpListener::bind(("127.0.0.1", PORT)).context("error binding the TcpListener")?;

    *state = State::Server(Vec::new());
    drop(state);

    thread::Builder::new()
        .name("TAS Editor Server Thread".to_string())
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

        STATE.lock().unwrap_server().push(RemoteGame {
            sender: hltas_sender,
            receiver: frames_receiver,
            state: RemoteGameState::Free,
        });
    }
}

/// Tries connecting to a remote server if not a server itself and not already connected.
///
/// Tries no more frequently than every second.
#[instrument(name = "remote::maybe_try_connecting_to_server", skip_all)]
pub fn maybe_try_connecting_to_server(marker: MainThreadMarker) {
    let mut state = STATE.lock();
    if !state.is_none() {
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

    static LAST_ATTEMPTED_AT: MainThreadCell<Option<Instant>> = MainThreadCell::new(None);
    if let Some(last_attempted_at) = LAST_ATTEMPTED_AT.get(marker) {
        if last_attempted_at.elapsed() < Duration::from_secs(1) {
            // One second hasn't elapsed yet.
            return;
        }
    }
    LAST_ATTEMPTED_AT.set(marker, Some(Instant::now()));

    let mut stream = match TcpStream::connect(("127.0.0.1", PORT)) {
        Ok(x) => x,
        Err(err) => {
            // Don't print an error if the server does not exist yet.
            if err.kind() != std::io::ErrorKind::ConnectionRefused {
                error!("Error connecting to the remote server: {err:?}");
            }

            return;
        }
    };

    let mut name = String::new();
    if let Err(err) = stream.read_to_string(&mut name) {
        error!("Error reading IPC name from the remote server: {err:?}");
        return;
    }
    drop(stream);

    let tx = match IpcSender::connect(name) {
        Ok(x) => x,
        Err(err) => {
            error!("Error connecting to the remote server IPC: {err:?}");
            return;
        }
    };

    let (hltas_sender, hltas_receiver) = match ipc_channel::ipc::channel() {
        Ok(x) => x,
        Err(err) => {
            error!("Error creating a HLTAS IPC channel: {err:?}");
            return;
        }
    };

    // Workaround for a Windows ipc-channel panic: the receiver for large payloads should be created
    // in the process that will be using it.
    //
    // https://github.com/servo/ipc-channel/issues/277
    let (workaround_sender, workaround_receiver) = match ipc_channel::ipc::channel() {
        Ok(x) => x,
        Err(err) => {
            error!("Error creating a workaround IPC channel: {err:?}");
            return;
        }
    };

    if let Err(err) = tx.send((hltas_sender, workaround_sender)) {
        error!("Error sending the IPC channels to the remote server: {err:?}");
        return;
    }

    let frames_sender = match workaround_receiver.recv() {
        Ok(x) => x,
        Err(err) => {
            error!("Error receiving the frames sender from the remote server: {err:?}");
            return;
        }
    };

    info!("Connected to a remote server.");

    *state = State::Client(RemoteServer {
        receiver: hltas_receiver,
        sender: frames_sender,
        simulation_state: SimulationState::Idle,
    });
}

pub fn maybe_disconnect_from_server(marker: MainThreadMarker) {
    if bxt::is_simulation_ipc_client(marker) {
        let mut state = STATE.lock();
        if state.is_client() {
            *state = State::None;
        }
    }
}

pub fn is_connected_to_server() -> bool {
    STATE.lock().is_client()
}

/// Receives any completed simulation results from the remote clients and calls `process_result` to
/// process them.
///
/// Note that the returned frames can contain less or more frames than there are in the HLTAS due to
/// currently inaccurate frame recording.
pub fn receive_simulation_result_from_clients(mut process_result: impl FnMut(HLTAS, Vec<Frame>)) {
    let mut state = STATE.lock();
    let games = state.unwrap_server();

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_busy()) {
        match game.try_recv_frames() {
            Ok(Some((hltas, frames))) => process_result(hltas, frames),
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

/// For all available (non-busy) remote clients, calls `prepare_hltas` to prepare a HLTAS and sends
/// it to the remote client for simulation.
///
/// If an error occurs sending the HLTAS to the remote client, it will be silently dropped without
/// simulating.
pub fn simulate_in_available_clients(mut prepare_hltas: impl FnMut() -> HLTAS) {
    let mut state = STATE.lock();
    let games = state.unwrap_server();

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_free()) {
        let hltas = prepare_hltas();

        if let Err(err) = game.start_simulating(hltas) {
            error!("Error sending HLTAS to a remote client: {err:?}");
            errored_indices.push(index);
        }
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }
}

/// Simulates the HLTAS on an available (non-busy) remote client.
///
/// Blocks until the simulation is complete.
///
/// Returns the simulation result on success and `None` if there were no clients able to serve the
/// request.
///
/// Note that the returned frames can contain less or more frames than there are in the HLTAS due to
/// currently inaccurate frame recording.
#[instrument(name = "remote::simulate", skip_all)]
pub fn simulate(hltas: HLTAS) -> Option<Vec<Frame>> {
    let mut state = STATE.lock();
    let games = state.unwrap_server();

    let mut result = None;

    let mut errored_indices = Vec::new();
    for (index, game) in games.iter_mut().enumerate().filter(|(_, g)| g.is_free()) {
        if let Err(err) = game.start_simulating(hltas.clone()) {
            error!("Error sending HLTAS to a remote client: {err:?}");
            errored_indices.push(index);
            continue;
        }

        let frames = match game.recv_frames() {
            Ok(frames) => frames,
            Err(err) => {
                error!("Error receiving simulation result from a remote client: {err:?}");
                errored_indices.push(index);
                continue;
            }
        };

        result = Some(frames);
        break;
    }

    for index in errored_indices.into_iter().rev() {
        games.remove(index);
    }

    result
}

pub fn receive_new_hltas_to_simulate() -> Option<HLTAS> {
    let mut state = STATE.lock();
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
    let mut state = STATE.lock();
    let server = match state.remote_server() {
        Some(x) => x,
        None => return,
    };

    let pending_frames = match &mut server.simulation_state {
        SimulationState::Recording(frames) => frames,
        _ => return,
    };

    let frame = get_frame_data();

    pending_frames.push(frame);
}

pub fn send_simulation_result_to_server() {
    let mut state = STATE.lock();
    let server = match state.remote_server() {
        Some(x) => x,
        None => return,
    };

    // Send empty frames if needed to avoid softlocks in unforeseen situations.
    let pending_frames = server.simulation_state.take_frames().unwrap_or_default();

    if let Err(err) = server.sender.send(pending_frames) {
        error!("Error when trying to send frames to the server: {err:?}");
        *state = State::None;
    }
}

pub fn start_recording_frames() {
    let mut state = STATE.lock();
    let server = match state.remote_server() {
        Some(x) => x,
        None => return,
    };

    if server.simulation_state.is_waiting_to_start() {
        server.simulation_state = SimulationState::Recording(Vec::new());
    }
}
