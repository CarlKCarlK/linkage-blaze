#!/usr/bin/env python3
"""Serve the current directory with cache disabled for all responses."""

from __future__ import annotations

import http.server
import socketserver
import sys


def parse_port(argv: list[str]) -> int:
    if len(argv) < 2:
        return 8081
    return int(argv[1])


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
    port = parse_port(sys.argv)
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("", port), NoCacheHandler) as httpd:
        print(f"Serving no-cache HTTP on 0.0.0.0 port {port}")
        httpd.serve_forever()
