scape = require("scape")

scape.on_startup(function()
	scape.spawn("wezterm")
end)

local space = "main"

scape.on_connector_change(function(outputs)
	local main_output = outputs[1]
	main_output.x = 0
	main_output.y = 0
	main_output.width = outputs[1].width
	main_output.height = outputs[1].height
	main_output.default = true
	main_output.disabled = false
	main_output.scale = 1

	scape.set_layout({
		[space] = {
			main_output,
		},
	})
	scape.set_zones({
		{
			name = "left",
			x = 0,
			y = 0,
			width = outputs[1].width / 4,
			height = outputs[1].height,
		},
		{
			name = "mid",
			x = outputs[1].width / 4 + 1,
			y = 0,
			width = outputs[1].width / 2,
			height = outputs[1].height,
			default = true,
		},
		{
			name = "right",
			x = outputs[1].width / 4 * 3 + 1,
			y = 0,
			width = outputs[1].width / 4,
			height = outputs[1].height,
		},
	})
end)

scape.map_key({
	key = "a",
	mods = "shift|super",
	callback = function()
		scape.spawn("wezterm")
	end,
})
scape.map_key({
	key = "b",
	mods = "shift|super",
	callback = function()
		scape.move_to_zone("left")
	end,
})
scape.map_key({
	key = "Left",
	mods = "shift",
	callback = function()
		scape.move_to_zone("left")
	end,
})
scape.map_key({
	key = "Left",
	mods = "super",
	callback = function()
		scape.move_to_zone("left")
	end,
})
scape.map_key({
	key = "Right",
	mods = "super",
	callback = function()
		scape.move_to_zone("right")
	end,
})
scape.map_key({
	key = "Up",
	mods = "super",
	callback = function()
		scape.move_to_zone("mid")
	end,
})
