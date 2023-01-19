use bxt_strafe::{Parameters, State};
use hltas::HLTAS;
use serde::{Deserialize, Serialize};

/// A movement frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Parameters used for simulating this frame.
    pub parameters: Parameters,

    /// Final state after this frame.
    pub state: State,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerRequest {
    /// Play this HLTAS without opening the editor.
    ///
    /// Used for secondary clients that play the TAS to send the accurate frames to the main client.
    Play(Play),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameResponse {}

/// Data for play request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Play {
    pub hltas: HLTAS,
    pub generation: u16,
}
