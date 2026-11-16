#!/bin/bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/target/release/Slopogram Native.app"
OUTPUT="$ROOT/target/release/Slopogram Native.dmg"
[ -d "$APP" ] || { printf 'Release app not found: %s\n' "$APP" >&2; exit 1; }
codesign --verify --deep --strict "$APP"
WORK=$(mktemp -d "$ROOT/target/release/.slopogram-dmg.XXXXXX")
MOUNTED=0
cleanup() {
    if [ "$MOUNTED" = 1 ]; then
        hdiutil detach "$WORK/mount" >/dev/null || { printf 'Could not detach %s\n' "$WORK/mount" >&2; return 1; }
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT
mkdir "$WORK/stage" "$WORK/mount"
ditto --noextattr --norsrc "$APP" "$WORK/stage/Slopogram Native.app"
ln -s /Applications "$WORK/stage/Applications"
cp "$ROOT/.env.example" "$WORK/stage/Slopogram.env.example"
hdiutil create -quiet -volname Slopogram -srcfolder "$WORK/stage" -format UDZO "$WORK/candidate.dmg"
hdiutil verify "$WORK/candidate.dmg" >/dev/null
hdiutil attach -readonly -nobrowse -mountpoint "$WORK/mount" "$WORK/candidate.dmg" >/dev/null
MOUNTED=1
MOUNTED_APP="$WORK/mount/Slopogram Native.app"
[ "$(readlink "$WORK/mount/Applications")" = /Applications ]
codesign --verify --deep --strict "$MOUNTED_APP"
python3 - "$APP" "$MOUNTED_APP" "$WORK/mount" <<'PY'
import hashlib, os, stat, subprocess, sys
from pathlib import Path

source, mounted, volume = map(Path, sys.argv[1:])
private_names = {'.env', 'tdata', 'key_datas', 'postbox', 'db.sqlite', 'db.sqlite-wal', 'db.sqlite-shm'}
if any(p.name.lower() in private_names or p.name.lower().endswith('.session') for p in mounted.rglob('*')):
    raise SystemExit('Found a private configuration or account data path in the packaged app.')
entries = {p.name for p in volume.iterdir()}
expected = {'Slopogram Native.app', 'Applications', 'Slopogram.env.example'}
if entries != expected:
    raise SystemExit(f'Unexpected image-root entries: {sorted(entries - expected)}; missing: {sorted(expected - entries)}')
template = (volume / 'Slopogram.env.example').read_text().splitlines()
assignments = [line.strip() for line in template if line.strip() and not line.lstrip().startswith('#')]
if assignments != ['TELEGRAM_API_ID=', 'TELEGRAM_API_HASH=']:
    raise SystemExit('The release configuration template must contain only empty API settings.')
print('Verified image root and blank configuration template.')
# The OS adds provenance metadata to copied/mounted files. Inspect attribute
# names only, never values; reject Finder metadata, resource forks, or extras.
attributes = subprocess.run(['xattr', '-r', str(volume)], check=True, capture_output=True, text=True).stdout.splitlines()
attribute_counts = {}
for line in attributes:
    name = line.rsplit(': ', 1)[-1]
    attribute_counts[name] = attribute_counts.get(name, 0) + 1
unexpected = set(attribute_counts) - {'com.apple.provenance'}
if unexpected:
    raise SystemExit(f'Unexpected extended attribute names: {sorted(unexpected)}')
print(f'OS-managed extended attribute names and counts: {attribute_counts}')

def manifest(root):
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            rel = str(path.relative_to(root))
            info = path.lstat()
            mode = stat.S_IMODE(info.st_mode)
            if stat.S_ISLNK(info.st_mode):
                result[rel] = ('link', mode, os.readlink(path))
            elif stat.S_ISDIR(info.st_mode):
                result[rel] = ('dir', mode)
            elif stat.S_ISREG(info.st_mode):
                digest = hashlib.sha256()
                with path.open('rb') as f:
                    for chunk in iter(lambda: f.read(1024 * 1024), b''):
                        digest.update(chunk)
                result[rel] = ('file', mode, digest.hexdigest())
            else:
                raise SystemExit(f'Unexpected file type: {rel}')
    return result

if manifest(source) != manifest(mounted):
    raise SystemExit('Mounted app differs from release app')
count = 0
for directory, _, files in os.walk(mounted):
    for name in files:
        path = Path(directory) / name
        kind = subprocess.run(['file', '-b', str(path)], check=True, capture_output=True, text=True).stdout
        if 'Mach-O' in kind:
            arches = subprocess.run(['lipo', '-archs', str(path)], check=True, capture_output=True, text=True).stdout.strip()
            if arches != 'arm64':
                raise SystemExit(f'Unexpected architecture in {path}: {arches}')
            count += 1
if not count:
    raise SystemExit('No Mach-O executables found')
print(f'Verified mounted app tree and {count} arm64 Mach-O files.')
PY
hdiutil detach "$WORK/mount" >/dev/null
MOUNTED=0
mv -f "$WORK/candidate.dmg" "$OUTPUT"
printf 'Built %s\n' "$OUTPUT"
