# Disclaimers

Deta doesn't specify how many instances an application can have in parallel. Also, they are free to
reboot an application. Hence, there are no guarantees for this file storage, and it is temporary.

# URLs

- /admin
- /read/read-only-dir-name
- /write/writeable-dir-hash

# Filesystem

We configure Warp (and Dav-Server) to follow symlinks. But, they don't list symlinks when
auto-generating directory listing (if enabled) or when listing over WebDAV. That is excellent for
us: We use it for need-to-know read-only access (as if directory listing were disabled at that
level). We still get directory listing for lower levels.

Write access is always on need-to-know basis. It's through a one way hash that can be re-generated.
The hash is based on the micro's private key (given by Deta), so it will stay constant even between
reboots.

| Filesystem Path                         | Immediate content updated by                   | Notes                    |
| --------------------------------------- | ---------------------------------------------- | ------------------------ |
| /tmp/                                   | ini                                            |                          |
| /tmp/wdav_dirs/                         | /admin                                         |                          |
| /tmp/wdav_dirs/dir-name/                | WebDAV                                         |                          |
|                                         |                                                |                          |
| /tmp/wdav_symlinks/                     | ini                                            |                          |
| /tmp/wdav_symlinks/CLEANUP_IN_PROGRESS  | cron (TODO)                                    |                          |
| /tmp/wdav_symlinks/write/               | /admin                                         | generated hash dir names |
| /tmp/wdav_symlinks/write/some-dir-hash/ | WebDAV (through /tmp/wdav_dirs/some-dir-name/) |                          |
| /tmp/wdav_symlinks/read/                | /admin                                         | given dir names          |
| /tmp/wdav_symlinks/read/some-dir-name/  | WebDAV (through /tmp/wdav_dirs/some-dir-name/) |                          |
| --------------------------------------- | ---------------------------------------------- | ------------------------ |

We use symlinks instead of hard links, even though it's all on the same filesystem (`/tmp`). That
way we prevent problems like upload-remote-reupload, where a hard link from under
`/tmp/wdav_symlinks/read/` could point to an obsolete file content already deleted, and new content
could be re-uploaded (via WebDAV) to `/tmp/wdav_dirs`, but the new uploaded file would have a
different file handle not linked to from `/tmp/wdav_symlinks/read/` (at least not linked until the
next `cron` run).

# Auto Cleanup (TODO)

We will have two types of cleanup

- orphan symlinks, and
- old files (when reaching quota).

We do NOT auto remove "old" empty directories, because we can't know the admin's intentions.
