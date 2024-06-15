alias r := run
alias rr := run-release
alias rw := run-winit
alias fg := flamegraph
alias fgg := flamegraph-graph

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
	cargo run --features profile-with-tracy --profile release-with-debug -- --config ./init.lua -l release.log

# run in release mode within a winit window with tracy
tracy-winit:
	cargo run --features profile-with-tracy --profile release-with-debug -- --winit-backend --config ./init.lua

# run in release mode with flamegraph
flamegraph:
	RUSTFLAGS='-C force-frame-pointers=y' cargo flamegraph --profile release-with-debug -- --config ./init.lua -l release.log

# run in release mode with flamegraph
flamegraph-graph:
	RUSTFLAGS='-C force-frame-pointers=y' cargo flamegraph -c "record -g" --profile release-with-debug -- --config ./init.lua -l release.log
