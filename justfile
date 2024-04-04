alias r := run
alias rr := run-release
alias rw := run-winit

#export RUST_LOG := "scape"

# run in dev mode
run:
	cargo run >> debug.log

# run in release mode
run-release:
	cargo run --release --features debug >> release.log -- --config ./init.lua

# run in dev mode within a winit window
run-winit:
	cargo run --features debug -- --winit-backend --config ./init.lua
