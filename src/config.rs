use crate::action::Action;
use crate::state::ActiveSpace;
use crate::State;
use anyhow::anyhow;
use calloop::LoopHandle;
use mlua::prelude::*;
use mlua::Table;
use smithay::output::Output;
use smithay::output::Scale;
use std::collections::HashMap;
use std::fs;
use tracing::info;
use tracing::warn;

#[derive(Debug)]
pub struct Config {
    lua: Lua,
    on_startup: Option<LuaFunction<'static>>,
    on_connector_change: Option<LuaFunction<'static>>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            lua: Lua::new(),
            on_startup: None,
            on_connector_change: None,
        }
    }
}

impl State {
    pub fn load_config(&mut self) -> anyhow::Result<()> {
        load_lua_config(self)
    }

    pub fn on_startup(&mut self) {
        info!("running on startup");
        if let Some(on_startup) = &self.config.on_startup {
            on_startup.call::<_, ()>(()).unwrap();
        }
    }

    pub fn on_connector_change(&mut self) {
        self.loop_handle.insert_idle(|state| {
            info!("running on connector change");
            if let Some(on_connector_change) = &state.config.on_connector_change {
                let config_outputs = state.outputs.values().map(Into::into).collect();

                on_connector_change
                    .call::<Vec<ConfigOutput>, ()>(config_outputs)
                    .unwrap();
            } else {
                info!("No on_connector_change callback set");
            }
        });
    }
}

const LUA_MODULE_NAME: &str = "scape";

fn load_lua_config(state: &mut State) -> anyhow::Result<()> {
    let loop_handle = state.loop_handle.clone();
    let _: Table = state.config.lua.load_from_function(
        LUA_MODULE_NAME,
        state
            .config
            .lua
            .create_function(move |lua: &Lua, _modname: String| {
                init_config_module(lua, loop_handle.clone())
            })?,
    )?;

    let user_config = fs::read("/home/dirli/.config/scape/init.lua")?;
    let config = state.config.lua.load(&user_config);
    let result = config.exec()?;
    Ok(result)
}

fn init_config_module<'lua>(
    lua: &'lua Lua,
    loop_handle: LoopHandle<'static, State>,
) -> LuaResult<LuaTable<'lua>> {
    let exports = lua.create_table()?;

    let lh = loop_handle.clone();
    exports.set(
        "on_startup",
        lua.create_function(move |_, callback: LuaFunction<'_>| {
            // SAFETY: The callback is valid as long as the lua instance is alive.
            // The lua instance is never dropped, therefore the lifetime of the callback is
            // effectively 'static.
            let callback: LuaFunction<'static> = unsafe { std::mem::transmute(callback) };
            lh.insert_idle(move |state| {
                state.config.on_startup = Some(callback);
            });
            Ok(())
        })?,
    )?;

    let lh = loop_handle.clone();
    exports.set(
        "on_connector_change",
        lua.create_function(move |_, callback: LuaFunction<'_>| {
            info!("Setting up on_connector_change");
            // SAFETY: The callback is valid as long as the lua instance is alive.
            // The lua instance is never dropped, therefore the lifetime of the callback is
            // effectively 'static.
            let callback: LuaFunction<'static> = unsafe { std::mem::transmute(callback) };
            lh.insert_idle(move |state| {
                state.config.on_connector_change = Some(callback);
            });
            Ok(())
        })?,
    )?;

    let lh = loop_handle.clone();
    exports.set(
        "spawn",
        lua.create_function(move |_, command| {
            lh.insert_idle(move |state| {
                state.execute(Action::Spawn { command });
            });
            Ok(())
        })?,
    )?;

    exports.set(
        "set_layout",
        lua.create_function(move |_, layout: ConfigLayout| {
            info!("New layout received");
            loop_handle.insert_idle(move |state| {
                info!("New layout will be set");
                for (space_name, config_outputs) in layout.spaces {
                    let space = state.spaces.entry(space_name.clone()).or_default();

                    for config_output in &config_outputs {
                        let Some(output) = state.outputs.get(&config_output.name) else {
                            warn!(output_name = %config_output.name, "Output not found");
                            continue;
                        };

                        let position = (config_output.x, config_output.y).into();
                        output.change_current_state(
                            None,
                            None,
                            Some(Scale::Integer(config_output.scale)),
                            Some(position),
                        );
                        space.map_output(&output, position);
                        if config_output.default {
                            output
                                .user_data()
                                .get_or_insert_threadsafe(|| ActiveSpace(space_name.clone()));
                        }
                    }

                    // clean up no longer mapped outputs
                    for (output_name, output) in &state.outputs {
                        if !config_outputs
                            .iter()
                            .any(|config_output| config_output.name == *output_name)
                        {
                            space.unmap_output(output);
                        }
                    }
                }

                // fixup window coordinates
                let location = state.pointer_location();
                for space in state.spaces.values_mut() {
                    crate::shell::fixup_positions(space, location);
                }

                state.start_outputs();
            });
            Ok(())
        })?,
    )?;

    Ok(exports)
}

struct ConfigLayout {
    spaces: HashMap<String, Vec<ConfigOutput>>,
}

impl<'lua> FromLua<'lua> for ConfigLayout {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        let table = value.as_table().unwrap().to_owned();

        let mut spaces = HashMap::new();
        for pair in table.pairs() {
            let (space_name, config_outputs) = pair.unwrap();

            spaces.insert(space_name, config_outputs);
        }

        Ok(ConfigLayout { spaces })
    }
}

struct ConfigOutput {
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    default: bool,
    disabled: bool,
    scale: i32,
}

impl From<&Output> for ConfigOutput {
    fn from(value: &Output) -> Self {
        let mode = value.preferred_mode().unwrap();
        let location = value.current_location();
        ConfigOutput {
            name: value.name(),
            x: location.x,
            y: location.y,
            width: mode.size.w,
            height: mode.size.h,
            default: true,   // FIXME: set proper value
            disabled: false, // FIXME: set proper value
            scale: value.current_scale().integer_scale(),
        }
    }
}

impl<'lua> IntoLua<'lua> for ConfigOutput {
    fn into_lua(self, lua: &'lua Lua) -> LuaResult<LuaValue<'lua>> {
        let lua_output = lua.create_table().unwrap();
        lua_output.set("name", self.name).unwrap();
        lua_output.set("x", self.x).unwrap();
        lua_output.set("y", self.y).unwrap();
        lua_output.set("width", self.width).unwrap();
        lua_output.set("height", self.height).unwrap();
        lua_output.set("default", self.default).unwrap();
        lua_output.set("disabled", self.disabled).unwrap();
        lua_output.set("scale", self.scale).unwrap();
        lua_output.into_lua(lua)
    }
}

impl<'lua> FromLua<'lua> for ConfigOutput {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        let table = value.as_table().unwrap();

        Ok(ConfigOutput {
            name: table.get("name").unwrap(),
            x: table.get("x").unwrap(),
            y: table.get("y").unwrap(),
            width: table.get("width").unwrap(),
            height: table.get("height").unwrap(),
            default: table.get("default").unwrap(),
            disabled: table.get("disabled").unwrap(),
            scale: table.get("scale").unwrap(),
        })
    }
}
