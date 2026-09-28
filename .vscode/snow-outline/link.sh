#!/bin/sh
set -e
src="$(cd "$(dirname "$0")" && pwd)"
ext="${1:-$HOME/.vscode-server/extensions}"
name=local.snow-outline-0.0.1
ln -sfn "$src" "$ext/$name"
python3 - "$ext" "$name" <<'EOF'
import json, sys, time
ext, name = sys.argv[1:]
path = f"{ext}/extensions.json"
entries = json.load(open(path))
if not any(e["identifier"]["id"] == "local.snow-outline" for e in entries):
    entries.append({
        "identifier": {"id": "local.snow-outline"},
        "version": "0.0.1",
        "location": {"$mid": 1, "path": f"{ext}/{name}", "scheme": "file"},
        "relativeLocation": name,
        "metadata": {"installedTimestamp": int(time.time() * 1000), "source": "vsix"},
    })
    json.dump(entries, open(path, "w"))
EOF
echo "linked $ext/$name, run 'Developer: Reload Window' to start using"
