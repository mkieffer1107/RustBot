"""Serve the demo's files, with cache headers so a Space update reaches visitors on a normal reload.

python -m http.server sends no Cache-Control, so browsers may keep an old index.html heuristically and pair it with
a newer worker.js or tree.json. Here the page, scripts and data are revalidated on every load (a cheap 304 when
unchanged), and only files whose names change with their content (the bot bundle) are cached for good.
"""
import os
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class Handler(SimpleHTTPRequestHandler):
    def end_headers(self):
        path = self.path.split("?", 1)[0]
        if path.endswith(".zip"):        # pybot-<sha12>-<hash>.zip: renamed on every rebuild
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        elif path.startswith("/fonts/"):
            self.send_header("Cache-Control", "public, max-age=86400")
        else:                            # index.html, worker.js, pybot_web.py, tree.json
            self.send_header("Cache-Control", "no-cache")
        super().end_headers()


if __name__ == "__main__":
    ThreadingHTTPServer((os.environ.get("HOST", "0.0.0.0"), int(os.environ.get("PORT", "7860"))), Handler).serve_forever()
