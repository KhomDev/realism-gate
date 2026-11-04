#!/usr/bin/env python3
"""Serves a directory over HTTPS on 127.0.0.1, for the release workflow's
cargo-binstall test (binstall refuses plain HTTP). Standard library only.

Usage: serve-https.py ROOT PORT CERT.pem KEY.pem
"""

from __future__ import annotations

import functools
import http.server
import ssl
import sys


def main(argv: list[str]) -> int:
    if len(argv) != 5:
        print(__doc__, file=sys.stderr)
        return 2
    root, port, cert, key = argv[1], int(argv[2]), argv[3], argv[4]
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=root)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)
    server.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
