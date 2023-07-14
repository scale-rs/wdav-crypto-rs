WDAV
====

Quick start a webdav server

# 📦️ Install

```shell
cargo install wdav
```

# 🌐 Start a webdav server

```shell
wdav.exe
```
```text
current dir : D:\Developments\Projects\wdav
listening on 0.0.0.0:8080 serving .
```

# 🦮 Print help

```shell
wdav.exe
```
```text
Quick start a webdav server

Usage: wdav.exe [OPTIONS]

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

Such as 
[GNOME Files application](docs/clients.md#gnome-files-application) ,
[Firefox](docs/clients.md#Firefox) , 
[Floccus](docs/clients.md#Floccus) ,
[Floccus with uBlock Origin](docs/clients.md#Floccus-with-uBlock-Origin) 


