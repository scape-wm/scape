alias r := run
alias rr := run-release
alias rw := run-winit

# run in dev mode
run:
	cargo run >> debug.log

# run in release mode
run-release:
	cargo run --release >> release.log

# run in dev mode within a winit window
run-winit:
	cargo run -- --winit-backend
