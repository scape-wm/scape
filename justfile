alias r := run
alias rw := run-winit

# run in dev mode
run:
	cargo run

# run in dev mode within a winit window
run-winit:
	cargo run -- --winit-backend
