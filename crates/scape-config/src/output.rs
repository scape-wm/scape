use calloop::LoopHandle;
use mlua::{
    Error as LuaError, FromLua, Function as LuaFunction, IntoLua, Lua, Result as LuaResult,
    Table as LuaTable, Value as LuaValue,
};
use scape_shared::Output;

use crate::ConfigState;

/// Set the output functions on the base module
pub(crate) fn init(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    module.set(
        "on_connector_change",
        lua.create_function(move |_, callback: LuaFunction| {
            loop_handle.insert_idle(move |state| {
                state.on_connector_change = Some(state.callback_state.register_callback(callback));
            });
            Ok(())
        })?,
    )?;

    Ok(())
}

impl ConfigState {
    pub(crate) fn on_connector_change(&mut self) -> anyhow::Result<()> {
        if let Some(on_connector_change) = &self.on_connector_change {
            return self.callback_state.run_callback(on_connector_change, ());
        }

        Ok(())
    }
}

struct ConfigOutput {
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<&Output> for ConfigOutput {
    fn from(value: &Output) -> Self {
        let mode = value.preferred_mode.unwrap(); // TODO: Construct default mode
        let location = value.location;
        ConfigOutput {
            name: value.name.clone(),
            x: location.x,
            y: location.y,
            width: mode.size.w,
            height: mode.size.h,
        }
    }
}

impl IntoLua for ConfigOutput {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let lua_output = lua.create_table()?;
        lua_output.set("name", self.name)?;
        lua_output.set("x", self.x)?;
        lua_output.set("y", self.y)?;
        lua_output.set("width", self.width)?;
        lua_output.set("height", self.height)?;
        lua_output.into_lua(lua)
    }
}

impl FromLua for ConfigOutput {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let table = value
            .as_table()
            .ok_or_else(|| LuaError::FromLuaConversionError {
                from: "LuaOutput",
                to: String::from("ConfigOutput"),
                message: Some(String::from("Expected a Lua table for the ConfigOutput")),
            })?;

        Ok(ConfigOutput {
            name: table.get("name")?,
            x: table.get("x")?,
            y: table.get("y")?,
            width: table.get("width")?,
            height: table.get("height")?,
        })
    }
}
