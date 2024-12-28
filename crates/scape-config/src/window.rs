use calloop::LoopHandle;
use mlua::{
    Error as LuaError, FromLua, Lua, Result as LuaResult, Table as LuaTable, Value as LuaValue,
};
use scape_shared::{DisplayMessage, WindowRule};

use crate::ConfigState;

pub(crate) fn init(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    init_add_window_rule(lua, module, loop_handle.clone())?;
    init_close_current_window(lua, module, loop_handle)?;

    Ok(())
}

fn init_close_current_window(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    module.set(
        "close_current_window",
        lua.create_function(move |_, ()| {
            loop_handle.insert_idle(move |state| {
                state.comms.display(DisplayMessage::CloseCurrentWindow);
            });
            Ok(())
        })?,
    )?;

    Ok(())
}

fn init_add_window_rule(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    module.set(
        "add_window_rule",
        lua.create_function(move |_, window_rule: ConfigWindowRule| {
            loop_handle.insert_idle(move |state| {
                state
                    .comms
                    .display(DisplayMessage::AddWindowRule(window_rule.into()));
            });
            Ok(())
        })?,
    )?;

    Ok(())
}

struct ConfigWindowRule {
    app_id: String,
    zone: String,
}

impl FromLua for ConfigWindowRule {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let table = value
            .as_table()
            .ok_or_else(|| LuaError::FromLuaConversionError {
                from: "LuaWindowRule",
                to: String::from("ConfigWindowRule"),
                message: Some(String::from(
                    "Expected a Lua table for the ConfigWindowRule",
                )),
            })?;

        Ok(ConfigWindowRule {
            app_id: table.get("app_id")?,
            zone: table.get("zone")?,
        })
    }
}

impl From<ConfigWindowRule> for WindowRule {
    fn from(value: ConfigWindowRule) -> Self {
        Self {
            app_id: value.app_id,
            zone: value.zone,
        }
    }
}
