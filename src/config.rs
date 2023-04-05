use std::fs;

use anyhow::Result;
use rlua::Lua;

fn read_config() -> Result<()> {
    let lua = Lua::new();
    lua.context(|lua_ctx| {
        lua_ctx
            .load(&fs::read("~/.config/scape/init.lua")?)
            .exec()?;
        Ok(())
    })
}

