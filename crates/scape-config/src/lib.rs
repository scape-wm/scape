//! The config module is responsible for allowing the user to interact with the rest of the application.

#![warn(missing_docs)]

mod callback;
mod config_watcher;
mod keymap;
mod output;
mod spawn;
mod window;
mod zone;

use std::{collections::HashMap, fs};

use callback::CallbackState;
use calloop::{LoopHandle, LoopSignal};
use config_watcher::ConfigWatcher;
use mlua::{Function as LuaFunction, Lua, Result as LuaResult, Table as LuaTable};
use scape_shared::{
    CallbackRef, Comms, ConfigMessage, DisplayMessage, GlobalArgs, InputMessage, MainMessage,
    MessageRunner, Mods, Output,
};
use tracing::{error, warn};

/// Holds the state of the config module
pub struct ConfigState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, ConfigState>,
    lua: Lua,
    callback_state: CallbackState,
    on_startup: Option<CallbackRef>,
    on_connector_change: Option<CallbackRef>,
    outputs: HashMap<String, Output>,
    extra_env: HashMap<String, String>,
}

impl MessageRunner for ConfigState {
    type Message = ConfigMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, Self>,
        args: &GlobalArgs,
    ) -> anyhow::Result<Self> {
        let mut state = Self {
            comms,
            shutting_down: false,
            loop_handle,
            lua: Lua::new(),
            callback_state: CallbackState::new(),
            on_startup: None,
            on_connector_change: None,
            outputs: HashMap::new(),
            extra_env: HashMap::new(),
        };
        state.load_user_config(args)?;

        Ok(state)
    }

    fn handle_message(&mut self, message: ConfigMessage) -> anyhow::Result<()> {
        match message {
            ConfigMessage::Shutdown => {
                self.shutting_down = true;
            }
            ConfigMessage::RunCallback(callback_ref) => {
                self.callback_state
                    .run_callback::<(), ()>(callback_ref, ())?;
            }
            ConfigMessage::ForgetCallback(callback_ref) => {
                self.callback_state.forget_callback(callback_ref)
            }
            ConfigMessage::Startup => {
                if let Some(on_startup) = self.on_startup {
                    self.callback_state.run_callback::<(), ()>(on_startup, ())?;
                }
            }
            ConfigMessage::ConnectorChange(outputs) => {
                self.outputs.clear();
                for output in outputs {
                    self.outputs.insert(output.name.clone(), output);
                }
                self.on_connector_change()?;
            }
            ConfigMessage::ExtraEnv { name, value } => {
                self.extra_env.insert(name, value);
            }
            ConfigMessage::Spawn(command, args) => {
                self.spawn(&command, &args);
            }
        }

        Ok(())
    }

    fn on_dispatch_wait(&mut self, signal: &LoopSignal) {
        if self.shutting_down {
            signal.stop();
        }
    }
}

const LUA_MODULE_NAME: &str = "scape";

impl ConfigState {
    /// Initialize the lua state and starts requires some lua modules
    fn load_user_config(&mut self, args: &GlobalArgs) -> anyhow::Result<()> {
        let loop_handle = self.loop_handle.clone();
        let _: LuaTable = self
            .lua
            .load_from_function(
                LUA_MODULE_NAME,
                self.lua
                    .create_function(move |lua: &Lua, _modname: String| {
                        init_base_module(lua, loop_handle.clone())
                    })
                    .map_err(|err| anyhow::anyhow!("Unable to initialize base module: {err}"))?,
            )
            .map_err(|err| anyhow::anyhow!("Unable to initialize base module: {err}"))?;

        if let Err(err) = self.set_default_keymaps() {
            error!("Unable to set default keymaps: {err}");
        }

        if let Err(err) = self.run_and_watch_user_config(args) {
            warn!("Unable to run and watch user config: {err}");
        }

        Ok(())
    }

    fn run_and_watch_user_config(&mut self, args: &GlobalArgs) -> anyhow::Result<()> {
        if let Some(config_path) = &args.config {
            let user_config = fs::read(config_path.as_str())?;
            let config = self.lua.load(&user_config);
            config
                .exec()
                .map_err(|err| anyhow::anyhow!("Unable to run config: {err}"))?;
        } else {
            let xdg_dirs = xdg::BaseDirectories::with_prefix("scape").unwrap();
            for path in xdg_dirs.list_config_files("") {
                let user_config = fs::read(path)?;
                let config = self.lua.load(&user_config);
                config
                    .exec()
                    .map_err(|err| anyhow::anyhow!("Unable to run config: {err}"))?;
            }
            self.loop_handle
                .insert_source(
                    ConfigWatcher::new(xdg_dirs.get_config_home()),
                    |path, _, state| {
                        let user_config = fs::read(path).unwrap();
                        let config = state.lua.load(&user_config);
                        config.exec().unwrap();
                    },
                )
                .unwrap();
        }

        Ok(())
    }

    fn set_default_keymaps(&mut self) -> LuaResult<()> {
        let default_keymaps = [
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "backspace",
                create_shutdown_callback(&self.lua, self.loop_handle.clone())?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f1",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 1)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f2",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 2)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f3",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 3)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f4",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 4)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f5",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 5)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f6",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 6)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f7",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 7)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f8",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 8)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f9",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 9)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f10",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 10)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f11",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 11)?,
            ),
            (
                Mods {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
                "f12",
                create_vt_callback(&self.lua, self.loop_handle.clone(), 12)?,
            ),
        ];

        for (mods, key_name, callback) in default_keymaps {
            self.comms.input(InputMessage::Keymap {
                key_name: key_name.to_string(),
                mods,
                callback: self.callback_state.register_callback(callback),
            });
        }

        Ok(())
    }
}

/// Initialize the base scape lua module which is used by the user config to interact with the
/// window manager in a script-able and convenient way.
fn init_base_module(lua: &Lua, lh: LoopHandle<'static, ConfigState>) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    let loop_handle = lh.clone();
    module.set(
        "on_startup",
        lua.create_function(move |_, callback: LuaFunction| {
            loop_handle.insert_idle(move |state| {
                if let Some(on_startup) = state.on_startup {
                    state.callback_state.forget_callback(on_startup);
                }
                state.on_startup = Some(state.callback_state.register_callback(callback));
            });
            Ok(())
        })?,
    )?;
    let loop_handle = lh.clone();
    module.set(
        "toggle_debug_ui",
        lua.create_function(move |_, ()| {
            loop_handle.insert_idle(move |state| {
                state.comms.display(DisplayMessage::ToggleDebugUi);
            });
            Ok(())
        })?,
    )?;
    module.set("quit", create_shutdown_callback(lua, lh.clone())?)?;
    module.set("shutdown", create_shutdown_callback(lua, lh.clone())?)?;
    let loop_handle = lh.clone();
    module.set(
        "start_video_stream",
        lua.create_function(move |_, ()| {
            loop_handle.insert_idle(move |state| {
                state.comms.display(DisplayMessage::StartVideoStream);
            });
            Ok(())
        })?,
    )?;

    keymap::init(lua, &module, lh.clone())?;
    output::init(lua, &module, lh.clone())?;
    spawn::init(lua, &module, lh.clone())?;
    zone::init(lua, &module, lh.clone())?;
    window::init(lua, &module, lh.clone())?;

    Ok(module)
}

fn create_shutdown_callback(
    lua: &Lua,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<LuaFunction> {
    lua.create_function(move |_, ()| {
        loop_handle.insert_idle(move |state| {
            state.comms.main(MainMessage::Shutdown);
        });
        Ok(())
    })
}

fn create_vt_callback(
    lua: &Lua,
    loop_handle: LoopHandle<'static, ConfigState>,
    vt: i32,
) -> LuaResult<LuaFunction> {
    lua.create_function(move |_, ()| {
        loop_handle.insert_idle(move |state| {
            state.comms.display(DisplayMessage::VtSwitch(vt));
        });
        Ok(())
    })
}
