#!/usr/bin/env python3
"""ZPNet routing controller. Emits JSON and never invokes a shell."""
from __future__ import annotations

import argparse, ipaddress, json, os, subprocess, sys, tempfile, time
from pathlib import Path
from urllib.request import ProxyHandler, build_opener

try:
    import fcntl
except ImportError:  # Pure planning tests also run on Windows; production target is Linux.
    fcntl = None

CONFIG = Path(os.environ.get("ZPNET_ROUTING_CONFIG", "/etc/zpnet-routing/config.json"))
STATE = Path(os.environ.get("ZPNET_ROUTING_STATE", "/var/lib/zpnet-routing/state.json"))
LOCK = Path(os.environ.get("ZPNET_ROUTING_LOCK", "/run/lock/zpnet-routing.lock"))

def load(path, default):
    try: return json.loads(path.read_text())
    except FileNotFoundError: return default

def atomic(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(dir=path.parent, prefix=".state-", text=True)
    try:
        with os.fdopen(fd, "w") as f:
            json.dump(data, f, ensure_ascii=False, indent=2); f.flush(); os.fsync(f.fileno())
        os.chmod(name, 0o600); os.replace(name, path)
    finally:
        if os.path.exists(name): os.unlink(name)

def run(argv, check=True):
    result = subprocess.run(argv, text=True, capture_output=True, timeout=15)
    if check and result.returncode: raise RuntimeError(f"{' '.join(argv)}: {result.stderr.strip()}")
    return result.stdout.strip()

def validated_cidrs(text, minimum=1, maximum=200000):
    result=[]
    for raw in text.splitlines():
        value=raw.split("#",1)[0].strip()
        if not value: continue
        network=ipaddress.ip_network(value, strict=False)
        if network.is_private or network.is_loopback or network.is_link_local or network.is_multicast or network.is_unspecified or network.is_reserved:
            raise ValueError(f"unsafe CIDR: {network}")
        result.append(str(network))
    result=sorted(set(result))
    if not minimum <= len(result) <= maximum: raise ValueError(f"CIDR count {len(result)} is outside {minimum}..{maximum}")
    return result

def update_ipsets(cfg, dry=False):
    plans=[]; opener=build_opener(ProxyHandler({}))
    for item in cfg.get("ipsets",[]):
        name=item["name"]
        if not name.replace("_","-").isalnum(): raise ValueError("unsafe ipset name")
        body=opener.open(item["url"],timeout=10).read(int(item.get("max_bytes",4000000))).decode()
        cidrs=validated_cidrs(body,int(item.get("min_entries",1)),int(item.get("max_entries",200000)))
        temp=f"{name}-next"; previous=f"{name}-previous"
        plans.append({"name":name,"entries":len(cidrs),"source":item["url"]})
        if dry: continue
        run(["ipset","destroy",temp],False); run(["ipset","create",temp,"hash:net","family","inet"])
        try:
            for cidr in cidrs: run(["ipset","add",temp,cidr])
            run(["ipset","destroy",previous],False)
            run(["ipset","swap",temp,name]); run(["ipset","rename",temp,previous])
        except Exception:
            run(["ipset","destroy",temp],False); raise
    return plans

def validate(cfg):
    net = ipaddress.ip_network(cfg["client_subnet"])
    if net.version != 4 or net.prefixlen < 16: raise ValueError("client_subnet is too broad")
    exits = cfg.get("exits", [])
    if len(exits) < 2: raise ValueError("at least two exits are required")
    ids, tables = set(), set()
    for e in exits:
        if e["id"] in ids or int(e["table"]) in tables: raise ValueError("duplicate exit id/table")
        ids.add(e["id"]); tables.add(int(e["table"]))
        ipaddress.ip_address(e["peer_ip"]); ipaddress.ip_address(e["public_ip"])
        if not str(e["health_url"]).startswith(f"http://{e['peer_ip']}:"):
            raise ValueError(f"{e['id']}: health_url must use tunnel peer_ip")
    return net, exits

def initial():
    return {"automatic": False, "clients": {}, "health": {}, "routes": {}, "events": [], "previous": None, "updated_at": 0}

def handshake(interface):
    out = run(["wg", "show", interface, "latest-handshakes"], False)
    values = [int(line.split()[-1]) for line in out.splitlines() if line.split() and line.split()[-1].isdigit()]
    return max(values, default=0) or None

def probe(exit_cfg):
    started = time.monotonic(); ok = False; detail = "health endpoint unavailable"
    try:
        request = build_opener(ProxyHandler({})).open(exit_cfg["health_url"], timeout=float(exit_cfg.get("timeout", 3)))
        payload = json.loads(request.read(8192)); seen = payload.get("public_ip") or payload.get("ip")
        ok = request.status == 200 and seen == exit_cfg["public_ip"]
        detail = "egress verified" if ok else f"unexpected egress: {seen or 'missing'}"
    except Exception as exc: detail = str(exc)[:160]
    return ok, round((time.monotonic() - started) * 1000), detail, handshake(exit_cfg["interface"])

def select_routes(exits, health):
    healthy = [e for e in exits if health.get(e["id"], {}).get("healthy")]
    if not healthy: return {str(e["table"]): None for e in exits}, "all exits are unavailable; fail-closed"
    main = next((e for e in healthy if e.get("role") == "main"), healthy[0])
    media = next((e for e in healthy if e.get("role") in ("youtube", "media", "nl")), main)
    routes = {}
    for e in exits: routes[str(e["table"])] = (media if e.get("role") in ("youtube", "media", "nl") else main)["id"]
    return routes, f"main={main['id']}, media={media['id']}"

def desired_commands(cfg, state, routes):
    _, exits = validate(cfg); by_id = {e["id"]: e for e in exits}; commands = []
    for table, exit_id in routes.items():
        if exit_id is None: commands.append(["ip", "route", "replace", "blackhole", "default", "table", table])
        else:
            e = by_id[exit_id]
            commands.append(["ip", "route", "replace", "default", "via", e["peer_ip"], "dev", e["interface"], "table", table])
    # One manager-owned rule range. Existing rules are normalized by reconcile_rules.
    commands += [
        ["ip", "rule", "add", "priority", str(cfg.get("direct_priority", 50)), "fwmark", cfg.get("direct_mark", "0x66"), "lookup", "main"],
        ["ip", "rule", "add", "priority", str(cfg.get("media_priority", 60)), "fwmark", cfg.get("media_mark", "0x77"), "lookup", str(cfg["media_table"])],
    ]
    for ip, mode in sorted(state["clients"].items()):
        if mode != "auto":
            target = next(e for e in exits if e["id"] == mode)
            commands.append(["ip", "rule", "add", "priority", "110", "from", f"{ip}/32", "lookup", str(target["table"])])
    commands.append(["ip", "rule", "add", "priority", str(cfg.get("default_priority", 998)), "from", cfg["client_subnet"], "lookup", str(cfg["main_table"])])
    return commands

def reconcile_rules(cfg, commands, dry):
    # Delete only signatures/priorities reserved by this manager, never unrelated rules.
    current = run(["ip", "-o", "rule", "show"], False)
    priorities = {str(cfg.get("direct_priority", 50)), str(cfg.get("media_priority", 60)), "110", str(cfg.get("default_priority", 998))}
    deletes = []
    for line in current.splitlines():
        priority = line.split(":", 1)[0].strip()
        if priority in priorities: deletes.append(["ip", "rule", "del", "priority", priority])
    plan = commands[:len(cfg["exits"])] + deletes + commands[len(cfg["exits"]):]
    if not dry:
        for cmd in plan: run(cmd)
    return plan

def status_payload(cfg, state, reason="", dry=False):
    _, exits = validate(cfg); active = set(state.get("routes", {}).values())
    return {"mode": "automatic" if state["automatic"] else "manual-lock", "automatic": state["automatic"], "dry_run": dry,
      "reason": reason, "updated_at": state["updated_at"],
      "exits": [{"id":e["id"],"name":e["name"],"public_ip":e["public_ip"],"role":e.get("role","fallback"),
        "healthy":state["health"].get(e["id"],{}).get("healthy",False),"latency_ms":state["health"].get(e["id"],{}).get("latency_ms"),
        "last_handshake":state["health"].get(e["id"],{}).get("last_handshake"),"current":e["id"] in active,
        "detail":state["health"].get(e["id"],{}).get("detail")} for e in exits],
      "clients":[{"ip":ip,"name":None,"mode":mode,"effective_exit": state.get("routes",{}).get(str(next((e["table"] for e in exits if e["id"]==mode), cfg["main_table"])))} for ip,mode in sorted(state["clients"].items())],
      "events":state["events"][-100:]}

def reconcile(cfg, state, dry=False):
    _, exits = validate(cfg); now = int(time.time()); new_health = dict(state["health"])
    for e in exits:
        ok, latency, detail, last = probe(e); old = new_health.get(e["id"], {})
        good = (old.get("successes",0)+1) if ok else 0; bad = 0 if ok else old.get("failures",0)+1
        healthy = old.get("healthy", False)
        if good >= int(cfg.get("success_threshold",3)): healthy=True
        if bad >= int(cfg.get("failure_threshold",3)): healthy=False
        new_health[e["id"]] = {"healthy":healthy,"successes":good,"failures":bad,"latency_ms":latency,"detail":detail,"last_handshake":last}
    candidate, reason = select_routes(exits, new_health)
    routes = candidate if state["automatic"] else state.get("routes", {}) or candidate
    next_state = dict(state); next_state["health"] = new_health
    plan = desired_commands(cfg, next_state, routes); applied = reconcile_rules(cfg, plan, dry)
    if not dry:
        if routes != state.get("routes"):
            next_state["previous"]={"routes":state.get("routes",{}),"clients":state["clients"],"automatic":state["automatic"]}
            next_state["events"]=(state["events"]+[ {"at":now,"kind":"route-change","message":reason}])[-200:]
        next_state["routes"]=routes; next_state["updated_at"]=now; atomic(STATE,next_state)
    payload=status_payload(cfg,next_state,reason,dry); payload["plan"]=[" ".join(x) for x in applied]
    return payload

def main():
    parser=argparse.ArgumentParser(); sub=parser.add_subparsers(dest="cmd",required=True)
    sub.add_parser("status"); sub.add_parser("reconcile"); sub.add_parser("dry-run"); sub.add_parser("rollback")
    auto=sub.add_parser("automatic"); auto.add_argument("value",choices=["on","off"])
    client=sub.add_parser("client"); client.add_argument("action",choices=["set"]); client.add_argument("ip"); client.add_argument("mode")
    sets=sub.add_parser("ipset-update"); sets.add_argument("--dry-run",action="store_true")
    args=parser.parse_args(); cfg=load(CONFIG,None)
    if not cfg: raise RuntimeError(f"missing config: {CONFIG}")
    net, exits=validate(cfg); LOCK.parent.mkdir(parents=True,exist_ok=True)
    with LOCK.open("w") as lock:
      if fcntl is None: raise RuntimeError("routing agent requires Linux flock support")
      fcntl.flock(lock,fcntl.LOCK_EX); state=load(STATE,initial())
      if args.cmd=="status": result=status_payload(cfg,state)
      elif args.cmd in ("reconcile","dry-run"): result=reconcile(cfg,state,args.cmd=="dry-run")
      elif args.cmd=="automatic":
        before={"routes":dict(state["routes"]),"clients":dict(state["clients"]),"automatic":state["automatic"]}
        state["automatic"]=args.value=="on"; result=reconcile(cfg,state)
        saved=load(STATE,state); saved["previous"]=before; atomic(STATE,saved)
      elif args.cmd=="client":
        ip=str(ipaddress.ip_address(args.ip)); mode=args.mode
        if ipaddress.ip_address(ip) not in net or ip.endswith(".0") or ip.endswith(".1") or ip.endswith(".255"): raise ValueError("client IP is outside allowed host range")
        if mode not in (["auto"]+[e["id"] for e in exits]): raise ValueError("unknown routing mode")
        before={"routes":dict(state["routes"]),"clients":dict(state["clients"]),"automatic":state["automatic"]}
        state["clients"][ip]=mode; result=reconcile(cfg,state)
        saved=load(STATE,state); saved["previous"]=before; atomic(STATE,saved)
      elif args.cmd=="ipset-update":
        plans=update_ipsets(cfg,args.dry_run); result=status_payload(cfg,state,"ipset validation complete",args.dry_run); result["ipsets"]=plans
      else:
        previous=state.get("previous")
        if not previous: raise RuntimeError("rollback snapshot is absent")
        state["routes"],state["clients"],state["automatic"]=previous["routes"],previous["clients"],previous["automatic"]
        plan=desired_commands(cfg,state,state["routes"]); reconcile_rules(cfg,plan,False)
        state["previous"]=None; state["updated_at"]=int(time.time()); state["events"]=(state["events"]+[{"at":state["updated_at"],"kind":"rollback","message":"previous routing state restored"}])[-200:]
        atomic(STATE,state); result=status_payload(cfg,state,"rollback applied")
    print(json.dumps(result,ensure_ascii=False))

if __name__ == "__main__":
  try: main()
  except Exception as exc: print(json.dumps({"error":str(exc)},ensure_ascii=False)); sys.exit(1)
