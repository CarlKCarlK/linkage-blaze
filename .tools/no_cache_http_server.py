#!/usr/bin/env python3
"""Serve the current directory with cache disabled for all responses."""

from __future__ import annotations

import http.server
import socketserver
import sys


def parse_args(argv: list[str]) -> tuple[int, bool]:
    port = 8081
    choose_next_free = False

    for argument in argv[1:]:
        if argument == "--next-free":
            choose_next_free = True
        else:
            port = int(argument)

    return port, choose_next_free


class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        # Ignore conditional request headers so we always return fresh content.
        if "If-Modified-Since" in self.headers:
            del self.headers["If-Modified-Since"]
        if "If-None-Match" in self.headers:
            del self.headers["If-None-Match"]
        return super().send_head()

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()


if __name__ == "__main__":
    port, choose_next_free = parse_args(sys.argv)
    socketserver.ThreadingTCPServer.allow_reuse_address = True
    while True:
        try:
            with socketserver.ThreadingTCPServer(("", port), NoCacheHandler) as httpd:
                print(f"Serving no-cache HTTP on 0.0.0.0 port {port}")
                httpd.serve_forever()
        except OSError as error:
            if not choose_next_free or error.errno != 98:
                raise
            port += 1
            continue
