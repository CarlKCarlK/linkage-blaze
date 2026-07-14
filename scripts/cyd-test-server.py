#!/usr/bin/env python3
"""Serve Linkage Blaze and Device Envoy CYD pages for browser tests."""

from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit
import sys


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
LINKAGE_PAGES = REPOSITORY_ROOT / "target" / "pages"
DEVICE_ENVOY_DNS = REPOSITORY_ROOT.parent / "mcu" / "device-envoy" / "docs" / "dns-tester" / "v1"


class CydTestHandler(SimpleHTTPRequestHandler):
    def translate_path(self, path):
        request_path = unquote(urlsplit(path).path)
        if request_path == "/dns-tester" or request_path.startswith("/dns-tester/"):
            root = DEVICE_ENVOY_DNS
            relative_path = request_path.removeprefix("/dns-tester/")
        else:
            root = LINKAGE_PAGES
            relative_path = request_path.lstrip("/")

        candidate = (root / relative_path).resolve()
        if candidate != root and root not in candidate.parents:
            return str(root)
        return str(candidate)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8092
    server = ThreadingHTTPServer(("127.0.0.1", port), CydTestHandler)
    print(f"Serving CYD test pages on http://127.0.0.1:{server.server_port}/", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
