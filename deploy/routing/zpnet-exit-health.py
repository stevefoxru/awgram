#!/usr/bin/env python3
"""Tiny EXIT-local health endpoint: success means traffic really leaves this EXIT."""
import json, os, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

EXPECTED = os.environ["ZPNET_EXPECTED_PUBLIC_IP"]
def egress_ip():
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    return json.loads(opener.open("https://api.ipify.org?format=json", timeout=4).read())["ip"]
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/health": self.send_error(404); return
        try: seen=egress_ip(); ok=seen==EXPECTED; body=json.dumps({"ok":ok,"public_ip":seen}).encode()
        except Exception as exc: ok=False; body=json.dumps({"ok":False,"error":str(exc)[:120]}).encode()
        self.send_response(200 if ok else 503); self.send_header("Content-Type","application/json"); self.send_header("Content-Length",str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *_): pass
ThreadingHTTPServer((os.environ.get("ZPNET_HEALTH_BIND","0.0.0.0"),int(os.environ.get("ZPNET_HEALTH_PORT","18081"))),Handler).serve_forever()
