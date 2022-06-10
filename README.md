# @

> instant rootless alpine shells

@ (atsi, "at-sign") is a tool for instantly provisioning interactive rootless
Alpine containers. Think of it a little like
[`nix-shell`](https://nixos.org/manual/nix/stable/command-ref/nix-shell.html)
for Alpine.

## Getting started

Static binaries are automatically released [here](https://github.com/queer/atsi/releases).

### DIY

Run [`cargo make debug`](https://github.com/sagiegurari/cargo-make) to create
a debug build, or run `cargo make release` to create a release build. Builds
are emitted to `target/{debug,release}/@`.

You may need to run `cargo install cargo-make` first.

## Basic commands

- `@ run`: Get an Alpine container running. Check `@ run --help` for all
           options.
- `@ ps`: Show all currently-running Alpine containers.

### Example outputs

<details>
  <summary>Terminal 1</summary>
  <pre><code>
git:(mistress) | ▶  @ run -p 8080:8081 -P python3 "python3 -m http.server 8081"
 INFO  atsi::engine::container > installing 1 package(s)...
 INFO  atsi::engine::container > requested packages: python3
fetch https://dl-cdn.alpinelinux.org/alpine/v3.16/main/x86_64/APKINDEX.tar.gz
fetch https://dl-cdn.alpinelinux.org/alpine/v3.16/community/x86_64/APKINDEX.tar.gz
v3.16.0-124-g321788a937 [https://dl-cdn.alpinelinux.org/alpine/v3.16/main]
v3.16.0-116-g0518fde496 [https://dl-cdn.alpinelinux.org/alpine/v3.16/community]
OK: 17022 distinct packages available
(1/13) Installing libbz2 (1.0.8-r1)
(2/13) Installing expat (2.4.8-r0)
(3/13) Installing libffi (3.4.2-r1)
(4/13) Installing gdbm (1.23-r0)
(5/13) Installing xz-libs (5.2.5-r1)
(6/13) Installing libgcc (11.2.1_git20220219-r2)
(7/13) Installing libstdc++ (11.2.1_git20220219-r2)
(8/13) Installing mpdecimal (2.5.1-r1)
(9/13) Installing ncurses-terminfo-base (6.3_p20220521-r0)
(10/13) Installing ncurses-libs (6.3_p20220521-r0)
(11/13) Installing readline (8.1.2-r0)
(12/13) Installing sqlite-libs (3.38.5-r0)
(13/13) Installing python3 (3.10.4-r0)
ERROR: 102 errors updating directory permissions
Executing busybox-1.35.0-r13.trigger
OK: 57 MiB in 27 packages
 INFO  atsi::engine::container > installed!
Serving HTTP on 0.0.0.0 port 8081 (http://0.0.0.0:8081/) ...
10.0.2.2 - - [10/Jun/2022 14:43:44] "GET / HTTP/1.1" 200 -
  </code></pre>
</details>

<details>
  <summary>Terminal 2</summary>
  <pre><code>
git:(mistress) | ▶  curl localhost:8080
&lt;!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01//EN" "http://www.w3.org/TR/html4/strict.dtd">
&lt;html>
&lt;head>
&lt;meta http-equiv="Content-Type" content="text/html; charset=utf-8">
&lt;title>Directory listing for /</title>
&lt;/head>
&lt;body>
&lt;h1>Directory listing for /</h1>
&lt;hr>
&lt;ul>
&lt;/ul>
&lt;hr>
&lt;/body>
&lt;/html>
git:(mistress) | ▶  
  </code></pre>
</details>

## How does it work?

Docker is a lot of effort, and frankly is overkill for this. Instead, @ creates
its own minimal containers, hooks up some basic networking, and extracts a
fresh Alpine rootfs.

[The process of setting up a container](https://github.com/queer/atsi/blob/51918281a42894690ec49fa6500b0d258ef02d93/src/engine/container.rs#L158-L228)
should be fairly legible.
