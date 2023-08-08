<!-- markdownlint-disable MD025 -->
# WDAV

Quick start a WebDAV server.

Based on [niuhuan/wdav-rs](https://github.com/niuhuan/wdav-rs).

# 📦️ Install

```shell
cargo build
```

or

```shell
cargo build --release
```

or use `cargo run` as per below.

# 🌐 Start a webdav server

If you did build, then

```shell
target/debug/wdav
```

or

```shell
target/release/wdav
```

or

```shell
cargo run
```

```text
current dir : D:\Developments\Projects\wdav
listening on 0.0.0.0:8080 serving .
```

# 🦮 Print help

```shell
target/debug/wdav --help
```

or

```shell
target/release/wdav --help
```

or

```shell
cargo run -- --help
```

```text
Quick start a webdav server

Usage: wdav [OPTIONS]

Options:
  -f, --folder <FOLDER>    Attach to webdav root [default: .]
  -a, --address <ADDRESS>  Address of listen [default: 0.0.0.0]
  -p, --port <PORT>        Port of listen [default: 8080]
  -h, --help               Print help
```

# 🔐 Security

Currently wdav does not support user authentication.

Either have that port denied by firewall, or ensure that you don't accidentally connect to a public
network.

Otherwise anyone on the same network can upload files and (in the better case) make your local user
hit its quota, or (in the worse case) fill up that partition.

# 🔗 Connecting with a client

You can use applications that support WebDav to access files on the current device.

Such as [GNOME Files application](docs/clients.md#gnome-files-application),
[Firefox](docs/clients.md#Firefox), [Floccus](docs/clients.md#Floccus), [Floccus with uBlock
Origin](docs/clients.md#Floccus-with-uBlock-Origin).
