.PHONY: build clean release static static-release

default: build

build:
	ssh-add && RUST_BACKTRACE=1 rustup run nightly cargo -Z package-features build ##--features="libsnark"

clean:
	rustup run nightly cargo clean

release:
	ssh-add && RUST_BACKTRACE=1 rustup run nightly cargo -Z package-features build --release ##--features="libsnark"

static: clean build
	rustc -g -O --crate-type staticlib target/debug/zokrates

static-release: clean release
	rustc -g -O --crate-type staticlib target/release/zokrates
