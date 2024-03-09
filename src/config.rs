use crate::action::Action;
use crate::State;
use calloop::LoopHandle;
use mlua::prelude::*;
use mlua::Table;
use std::fs;
use tracing::info;

#[derive(Debug)]
pub struct Config {
    lua: Lua,
    on_startup: Option<LuaFunction<'static>>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            lua: Lua::new(),
            on_startup: Default::default(),
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
}

fn load_lua_config(state: &mut State) -> anyhow::Result<()> {
    let loop_handle = state.loop_handle.clone();
    let _: Table = state.config.lua.load_from_function(
        "scape",
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
            let callback = unsafe { std::mem::transmute(callback) };
            lh.clone().insert_idle(move |state| {
                state.config.on_startup = Some(callback);
            });
            Ok(())
        })?,
    )?;

    exports.set(
        "spawn",
        lua.create_function(move |_, command| {
            loop_handle.clone().insert_idle(move |state| {
                state.execute(Action::Spawn { command });
            });
            Ok(())
        })?,
    )?;

    Ok(exports)
}
