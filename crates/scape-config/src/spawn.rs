use std::process::Command;

use calloop::LoopHandle;
use mlua::{
    Error as LuaError, FromLua, Lua, Result as LuaResult, Table as LuaTable, Value as LuaValue,
};
use tracing::{error, info};

use crate::ConfigState;

pub(crate) fn init(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    module.set(
        "spawn",
        lua.create_function(move |_, spawn: ConfigSpawn| {
            loop_handle.insert_idle(move |state| {
                state.spawn(&spawn.command, &spawn.args);
            });
            Ok(())
        })?,
    )?;

    Ok(())
}

impl ConfigState {
    fn spawn(&self, command: &str, args: &[String]) {
        info!(command, "Starting program");

        if let Err(e) = Command::new(command)
            .args(args)
            .envs(self.extra_env.iter())
            .spawn()
        {
            error!(command, err = %e, "Failed to start program");
        }
    }
}

struct ConfigSpawn {
    command: String,
    args: Vec<String>,
}

impl FromLua for ConfigSpawn {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let table = value
            .as_table()
            .ok_or_else(|| LuaError::FromLuaConversionError {
                from: "LuaSpawn",
                to: String::from("ConfigSpawn"),
                message: Some(String::from("Expected a Lua table for the ConfigSpawn")),
            })?;

        Ok(Self {
            command: table.get("command")?,
            args: table.get("args").unwrap_or_default(),
        })
    }
}
