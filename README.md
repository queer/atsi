# @

> instant rootless alpine shells

@ (atsi, "at-sign") is a tool for instantly provisioning interactive rootless
Alpine containers. Think of it a little like
[`nix-shell`](https://nixos.org/manual/nix/stable/command-ref/nix-shell.html)
for Alpine.

## Getting started

Run [`cargo make debug`](https://github.com/sagiegurari/cargo-make) to create
a debug build, or run `cargo make release` to create a release build. Builds
are emitted to `target/{debug,release}/@`.

You may need to run `cargo install cargo-make` first.

## Basic commands

- `@ run`: Get an Alpine container running. Check `@ run --help` for all
           options.
- `@ ps`: Show all currently-running Alpine containers.

## How does it work?

Docker is a lot of effort, and frankly is overkill for this. Instead, @ creates
its own minimal containers, hooks up some basic networking, and extracts a
fresh Alpine rootfs.

[The process of setting up a container](https://github.com/queer/atsi/blob/51918281a42894690ec49fa6500b0d258ef02d93/src/engine/container.rs#L158-L228)
should be fairly legible.
