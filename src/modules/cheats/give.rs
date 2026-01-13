//! `bxt_give`

use std::ffi::CString;

use crate::handler;
use crate::hooks::engine::{self, con_print, get_entity_index, is_cheat_enabled, player_edict};
use crate::modules::commands::{Command, Commands};
use crate::modules::cvars::CVars;
use crate::modules::Module;
use crate::utils::*;

pub struct CheatGive;
impl Module for CheatGive {
    fn name(&self) -> &'static str {
        "bxt_give"
    }

    fn description(&self) -> &'static str {
        "Retores functionality to give named items."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_GIVE];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        CVars.is_enabled(marker)
        && Commands.is_enabled(marker)
        && engine::cvar_vars.is_set(marker) // is_cheats_enabled
        && engine::AllocEngineString.is_set(marker)
        && engine::CreateNamedEntity.is_set(marker)
        && engine::gEntityInterface.is_set(marker)
    }
}

static BXT_GIVE: Command = Command::new(
    b"bxt_give\0",
    handler!(
        "bxt_give

Gives named item.",
        give_item as fn(_, _)
    ),
);

fn give_item(marker: MainThreadMarker, item: String) {
    if !CheatGive.is_enabled(marker) {
        return;
    }

    if unsafe { !is_cheat_enabled(marker) } {
        return;
    }

    // allocate engine string
    let Ok(cstring) = CString::new(item) else {
        return;
    };
    let item_str = unsafe { engine::AllocEngineString.get(marker)(cstring.as_ptr()) };

    // creating entity
    let new_entity = unsafe { engine::CreateNamedEntity.get(marker)(item_str) };
    let new_entity_index = unsafe { get_entity_index(marker, new_entity) };

    if new_entity.is_null() || new_entity_index.is_none() {
        con_print(marker, "bxt_give a NULL entity\n");
        return;
    }

    let new_entity = unsafe { &mut *new_entity };

    let player = unsafe { player_edict(marker) };
    let Some(player) = player.map(|mut player| unsafe { player.as_mut() }) else {
        return;
    };

    new_entity.v.origin = player.v.origin;
    new_entity.v.spawnflags |= 1 << 30; // SF_NORESPAWN

    // let the server know that we spawn things
    unsafe {
        let entity_interface = &*engine::gEntityInterface.get(marker);

        let Some(dispatch_spawn) = entity_interface.pfnSpawn else {
            con_print(marker, "No DispatchSpawn found for spawning entity\n");
            return;
        };

        let Some(dispatch_touch) = entity_interface.pfnTouch else {
            con_print(marker, "No DispatchTouch found for spawning entity\n");
            return;
        };

        dispatch_spawn(new_entity);
        dispatch_touch(new_entity, player);
    }
}
