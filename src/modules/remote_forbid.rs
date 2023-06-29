//! `bxt_remote_forbid`

use super::Module;
use crate::handler;
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct RemoteForbid;
impl Module for RemoteForbid {
    fn name(&self) -> &'static str {
        "bxt_remote_forbid"
    }

    fn description(&self) -> &'static str {
        "Forbidding this game from connecting as a remote client to other instances."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_REMOTE_FORBID];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
    }
}

static BXT_REMOTE_FORBID: Command = Command::new(
    b"bxt_remote_forbid\0",
    handler!(
        "bxt_remote_forbid [0|1]

Disconnects this game from other game instances, and forbids future connections. If called with \
argument 0, then lets the game connect again.",
        remote_forbid as fn(_),
        remote_forbid_with_arg as fn(_, _)
    ),
);

static FORBID: MainThreadCell<bool> = MainThreadCell::new(false);

fn remote_forbid(marker: MainThreadMarker) {
    remote_forbid_with_arg(marker, 1);
}

fn remote_forbid_with_arg(marker: MainThreadMarker, arg: u8) {
    FORBID.set(marker, arg != 0);
}

pub fn should_forbid(marker: MainThreadMarker) -> bool {
    FORBID.get(marker)
}
