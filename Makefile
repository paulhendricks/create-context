all: install

clean:
	cargo clean

build: clean
	cargo build

run: build
	cargo run

install: build
	cargo install --path .
