# soydrop

[![Workflow Status](https://github.com/artnoi43/soydrop/workflows/main/badge.svg)](https://github.com/artnoi43/soydrop/actions?query=workflow%3A%22main%22)
![Maintenance](https://img.shields.io/badge/maintenance-activly--developed-brightgreen.svg)

```text
____ ____ _   _ ___  ____ ____ ___
[__  |  |  \_/  |  \ |__/ |  | |__]
___] |__|   |   |__/ |  \ |__| |
```

soydrop is a simple web app for sharing texts across different computers.

## Features

soydrop writes text to file or in-memory clipboard store, with a timer.
The clipboard is later accessed by referencing the first 4 characters of
hex-encoded representation of its SHA2 hash.

For security reason, host it behind a firewall and VPN, or use modern reverse proxy
like NGINX to enable HTTP Basic Authentication.

- In-memory or file storage

- Multiple endpoints for different HTTP content type: HTML, JSON, and plain text

- Expiration timer (can be reset/extended)

- Configuation via files or environment

### Planned features (not yet implemented)

- Expandable hash keys using trie nodes for clipboard hashes (see branch `dev/trie`)

- AES or RSA encryption,

- File upload (probably with multiform)

- TCP support

## Why write soydrop in the first place?

Most of my computers run on different Linux distros, my servers on OpenBSD,
and my phone is iOS, which make it super difficult to share clipboards.

I could have used ready-made solution like PasteBin or email the text to myself,
but that would make me nervous when sending sensitive info like SSH keys.

And I want to try Rust anyway, so here it is.

Current version: 0.1.0

License: BSD-3-Clause OR GPL-2.0
