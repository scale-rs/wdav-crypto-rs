<!-- markdownlint-disable MD025 -->
# Temporary WebDav server

Quick start a WebDAV server. Deployable on [Deta.Space](https://deta.space/docs).

Based on [niuhuan/wdav-rs](https://github.com/niuhuan/wdav-rs) (which, on its own, is NOT deployable on Deta.Space).

# Installation, local run and deployment

# 🔐 Security

Anyone with a write hash can can upload files and, by doing so, can fill up `/tmp` partition,
potentially make the website unusable (and prevent others from uploading any files). If you remove
the folder for a (previously) generated write hash, no one can upload files through that hash
anymore.

# No Index.html, nor autoindexing for now

Even though `da-server` has API to enable/disable autoindexing and/or serving `index.html` (or
similar), it seems not to be working. To be investigated.
<!-- When accessing directory URLs under non-WebDAV (classic) HTTP, and if the directory contains
`index.html`, this returns it. That serves for previewing demos/snippets of static HTML websites.-->

# 🔗 Connecting with a client

You can use applications that support WebDav to access files on the current device.

Such as [GNOME Files application](docs/clients.md#gnome-files-application),
[Firefox](docs/clients.md#Firefox), [Floccus](docs/clients.md#Floccus), [Floccus with uBlock
Origin](docs/clients.md#Floccus-with-uBlock-Origin).

## Tests

Unfortunately, it seems that Rust [doesn't allow us to override this
default](https://github.com/rust-lang/rust/blob/41bafc4ff3eb6a73aa40e60c3bd4494302c7ec57/library/test/src/time.rs#L61).
