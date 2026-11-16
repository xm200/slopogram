#!/usr/bin/env python3
import pathlib
import re
import subprocess
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
upstream = root / 'native/telegram-macos'
ios = upstream / 'submodules/telegram-ios'
setup = (root / 'scripts/setup-direct-upstream.sh').read_text()
entries = re.findall(r'apply_patch_once "\$(UPSTREAM|IOS)" "\$PATCH_ROOT/([^"\n]+)"', setup)
entries.insert(0, ('IOS', str(upstream / 'scripts/telegram-ios-macos.patch')))
if not entries:
    raise SystemExit('No patch series found')
with tempfile.TemporaryDirectory(prefix='slopogram-patch-test-') as scratch:
    work = {'UPSTREAM': pathlib.Path(scratch) / 'app', 'IOS': pathlib.Path(scratch) / 'ios'}
    origins = {'UPSTREAM': upstream, 'IOS': ios}
    for folder in work.values():
        folder.mkdir()
    series = []
    seen = {'UPSTREAM': set(), 'IOS': set()}
    touched = {'UPSTREAM': set(), 'IOS': set()}
    for kind, name in entries:
        patch = pathlib.Path(name) if name.startswith('/') else root / 'native/telegram-patches' / name
        if not patch.exists() and name == '0002-ayu-telegram-ios.patch':
            continue
        raw = patch.read_text()
        touched[kind].update(re.findall(r'^(?:--- a/|\+\+\+ b/)(.+)$', raw, re.M))
        for rel in re.findall(r'^--- a/(.+)$', raw, re.M):
            if rel in seen[kind]:
                continue
            seen[kind].add(rel)
            destination = work[kind] / rel
            destination.parent.mkdir(parents=True, exist_ok=True)
            data = subprocess.check_output(['git', '-C', str(origins[kind]), 'show', 'HEAD:' + rel])
            destination.write_bytes(data)
            tree = subprocess.check_output(['git', '-C', str(origins[kind]), 'ls-tree', 'HEAD', '--', rel], text=True)
            if tree.startswith('100755 '):
                destination.chmod(0o755)
        series.append((kind, patch))
    for kind, patch in series:
        subprocess.run(['git', 'apply', '--check', str(patch)], cwd=work[kind], check=True)
        subprocess.run(['git', 'apply', str(patch)], cwd=work[kind], check=True)
    for kind, patch in series:
        subprocess.run(['git', 'apply', '--reverse', '--check', str(patch)], cwd=work[kind], check=True)
    if '--compare-working-tree' in sys.argv:
        mismatches = []
        for kind, paths in touched.items():
            for rel in sorted(paths):
                expected = work[kind] / rel
                actual = origins[kind] / rel
                if expected.exists() != actual.exists() or (expected.exists() and expected.read_bytes() != actual.read_bytes()):
                    mismatches.append(f'{kind}/{rel}')
        if mismatches:
            raise SystemExit('Working checkout differs from replayed patches:\n' + '\n'.join(mismatches))
        print('Working patched files match fresh replay.')
    print(f'Validated {len(series)} patches from pinned source and reverse checks.')
