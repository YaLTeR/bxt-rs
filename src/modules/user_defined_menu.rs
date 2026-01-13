//! `User Defined Custom Menu`

use std::fs::OpenOptions;
use std::io::Read;
use std::sync::Arc;

use serde::Deserialize;

use super::Module;
use crate::handler;
use crate::hooks::engine::{self, con_print, prepend_command};
use crate::modules::commands::{Command, Commands};
use crate::modules::menu::{self, CustomMenu, CustomMenuItem};
use crate::utils::*;

pub struct UserDefinedMenu;
impl Module for UserDefinedMenu {
    fn name(&self) -> &'static str {
        "User Defined Custom Menu"
    }

    fn description(&self) -> &'static str {
        "Loads and displays custom menu from a JSON file."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_MENU_LOAD];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        Commands.is_enabled(marker)
            && menu::Menu.is_enabled(marker)
            && engine::Cbuf_InsertText.is_set(marker) // prepend command
    }
}

static BXT_MENU_LOAD: Command = Command::new(
    b"bxt_menu_load\0",
    handler!(
        "bxt_menu_load

Loads and displays custom menu from a JSON file.

Example BXT checkpoint and timer commands.
```json
{
    \"label\": \"Checkpoint Menu\",
    \"items\": [
        {
            \"label\": \"CheckPoint\",
            \"command\": \"bxt_ch_checkpoint_create\"
        },
        {
            \"label\": \"GoCheck\",
            \"command\": \"bxt_ch_checkpoint_goto\"
        },
        {
            \"label\": \"Remove Checkpoint\",
            \"command\": \"bxt_ch_checkpoint_remove\"
        },
        {
            \"label\": \"Pause Timer\",
            \"command\": \"bxt_timer_stop\"
        },
        {
            \"label\": \"Start Timer\",
            \"command\": \"bxt_timer_start\"
        },
        null,
        {
            \"label\": \"Reset Timer\",
            \"command\": \"bxt_timer_reset\"
        },
        {
            \"label\": \"Echo Test 1\",
            \"options\": [
                \"It's black\",
                \"It's white\",
                \"It's black\",
                \"It's yeah yeah yeah\"
            ],
            \"command\": \"echo\"
        },
        null,
        null,
        {
            \"label\": \"Nested Menu\",
            \"items\": [
                {
                    \"label\": \"Quit\",
                    \"command\": \"quit\"
                },
                {
                    \"label\": \"map c1a0\",
                    \"command\": \"map c1a0\"
                }
            ]
        },
        {
            \"label\": \"Echo Test 2\",
            \"options\": [
                \"And\",
                \"potentially\",
                \"the\",
                \"most\",
                \"unstable!\"
            ],
            \"command\": \"echo hi\",
            \"exclude_option\": true
        }
    ]
}
```",
        load_menu as fn(_, _)
    ),
);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct JsonMenu {
    label: String,
    items: Vec<JsonMenuItem>,
}

#[derive(Debug, Deserialize)]
// untagged so that it is easier to write json
#[serde(untagged, rename_all = "snake_case")]
enum JsonMenuItem {
    SubMenu(JsonMenu),
    // need to put CycleCommand before Action
    // so that serde can deserialize correctly
    CycleCommand {
        label: String,
        options: Vec<String>,
        command: String,
        #[serde(default)]
        exclude_option: bool,
    },
    Action {
        label: String,
        command: String,
    },
    Empty,
}

impl JsonMenu {
    fn to_custom_menu(self) -> CustomMenu {
        CustomMenu {
            label: self.label,
            items: self
                .items
                .into_iter()
                .map(|item| item.to_custom_menu_item())
                .collect(),
        }
    }
}

impl JsonMenuItem {
    fn to_custom_menu_item(self) -> CustomMenuItem {
        match self {
            JsonMenuItem::SubMenu(json_menu) => CustomMenuItem::SubMenu(json_menu.to_custom_menu()),
            JsonMenuItem::CycleCommand {
                label,
                options,
                command,
                exclude_option,
            } => CustomMenuItem::CycleCommand {
                label,
                options,
                value: 0,
                command,
                exclude_option,
            },
            JsonMenuItem::Action { label, command } => CustomMenuItem::Action {
                label,
                callback: Arc::new(move |marker| {
                    // basically just execute console command/cvar
                    prepend_command(marker, format!("{}\n", command).as_str());
                }),
                extra_text: None,
            },
            JsonMenuItem::Empty => CustomMenuItem::Empty,
        }
    }
}

fn load_menu(marker: MainThreadMarker, file_name: String) {
    let Ok(mut file) = OpenOptions::new().read(true).open(&file_name) else {
        con_print(marker, format!("Cannot open file {}\n", file_name).as_str());
        return;
    };

    let mut s = String::new();
    let Ok(_) = file.read_to_string(&mut s) else {
        con_print(marker, "Cannot read file\n");
        return;
    };

    let Ok(s_deser) = serde_json::from_str::<JsonMenu>(&s) else {
        con_print(marker, "Failed to parse JSON menu file\n");
        return;
    };

    let user_menu = s_deser.to_custom_menu();

    menu::toggle_menu_display(marker, user_menu);
}
