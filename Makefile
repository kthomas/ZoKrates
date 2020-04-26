.PHONY: build clean static

default: build

build:
	RUST_BACKTRACE=1 rustup run nightly cargo -Z package-features build --release --features="libsnark"

clean:
	rustup run nightly cargo clean

release:
	RUST_BACKTRACE=1 rustup run nightly cargo -Z package-features build --debug --features="libsnark"

static: clean build
	rustc -g -O --crate-type staticlib target/debug/zokrates
	#rustc -g -O --crate-type staticlib target/release/zokrates

