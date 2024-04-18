alias r := run
alias rr := run-release
alias rw := run-winit

#export RUST_LOG := "scape"

# run in dev mode
run:
	cargo run >> debug.log

# run in release mode
run-release:
	cargo run --release --features debug -- --config ./init.lua -l release.log

# run in dev mode within a winit window
run-winit:
	cargo run --features debug -- --winit-backend --config ./init.lua

# run in release mode with tracy
tracy:
	cargo run --features profile-with-tracy --release -- --config ./init.lua -l release.log

# run in release mode within a winit window with tracy
tracy-winit:
	cargo run --features profile-with-tracy --release -- --winit-backend --config ./init.lua
