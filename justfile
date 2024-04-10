alias r := run
alias rr := run-release
alias rw := run-winit

#export RUST_LOG := "scape"

# run in dev mode
run:
	cargo run >> debug.log

# run in release mode
run-release:
	cargo run --release --features -- --config ./init.lua -l release.log

# run in dev mode within a winit window
run-winit:
	cargo run --features debug -- --winit-backend --config ./init.lua

# run in release mode with puffin
puffin:
	cargo run --features profile-with-puffin --release -- --config ./init.lua -l release.log

# run in release mode within a winit window with puffin
puffin-winit:
	cargo run --features profile-with-puffin --release -- --winit-backend --config ./init.lua

# run in release mode with tracy
tracy:
	cargo run --features profile-with-tracy --release -- --config ./init.lua -l release.log

# run in release mode within a winit window with tracy
tracy-winit:
	cargo run --features profile-with-tracy --release -- --winit-backend --config ./init.lua
