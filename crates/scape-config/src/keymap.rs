use calloop::LoopHandle;
use mlua::{
    FromLua, Function as LuaFunction, Lua, Result as LuaResult, Table as LuaTable,
    Value as LuaValue,
};
use scape_shared::{InputMessage, Mods};
use tracing::warn;

use crate::ConfigState;

pub(crate) fn init(
    lua: &Lua,
    module: &LuaTable,
    loop_handle: LoopHandle<'static, ConfigState>,
) -> LuaResult<()> {
    module.set(
        "map_key",
        lua.create_function(move |_, spawn: ConfigKeymap| {
            loop_handle.insert_idle(move |state| {
                state.comms.input(InputMessage::Keymap {
                    key_name: spawn.key,
                    mods: spawn.mods,
                    callback: state.callback_state.register_callback(spawn.callback),
                });
            });
            Ok(())
        })?,
    )?;

    Ok(())
}

struct ConfigKeymap {
    key: String,
    mods: Mods,
    callback: LuaFunction,
}

impl FromLua for ConfigKeymap {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let table = value.as_table().unwrap();

        let mut mods = Mods::default();
        for mod_key in table.get::<String>("mods").unwrap_or_default().split('|') {
            match mod_key {
                "shift" => mods.shift = true,
                "logo" | "super" => mods.logo = true,
                "ctrl" => mods.ctrl = true,
                "alt" => mods.alt = true,
                "" => {}
                _ => warn!(%mod_key, "Unhandled mod key"),
            }
        }

        let key = table.get::<String>("key")?;
        let callback = table.get::<LuaFunction>("callback")?;

        Ok(ConfigKeymap {
            key,
            mods,
            callback,
        })
    }
}
