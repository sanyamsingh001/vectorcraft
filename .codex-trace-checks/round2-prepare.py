import difflib, hashlib, json, os, pathlib, shutil, subprocess
root = pathlib.Path(__file__).resolve().parent.parent
out = root / '.codex-trace-checks'
base_sha = subprocess.check_output(['git', 'rev-parse', 'c26c4a2'], cwd=root, text=True).strip()
changed = ['ROADMAP.md', 'crates/trace/src/contour.rs', 'crates/trace/src/quantize.rs']
trace_files = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', base_sha, '--', 'crates/trace/src'], cwd=root, text=True).splitlines()
assert all(f.startswith('crates/trace/src/') for f in trace_files)
files = ['ROADMAP.md', *trace_files]
def git(*args):
    return subprocess.check_output(['git', *args], cwd=root)
def digest(data):
    return hashlib.sha256(data).hexdigest()
guard = digest(git('diff', '--binary', 'HEAD'))
manifest = {'base': base_sha, 'source_guard_before': guard, 'changed_paths': changed}
base = out / 'round2-c26-base'
candidate = out / 'round2-c26-optimized'
staged = out / 'round2-c26-staged'
for directory in [base, candidate, staged]:
    directory.mkdir(exist_ok=False)
    for f in files:
        dest = directory / f
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(git('show', base_sha + ':' + f))
staged_roadmap = git('show', ':ROADMAP.md')
# Use the canonical index read-only, via the known canonical root.
staged_roadmap = subprocess.check_output(['git', 'show', ':ROADMAP.md'], cwd='/mnt/downloads/dev/artcraft/vectorcraft')
(staged / 'ROADMAP.md').write_bytes(staged_roadmap)
manifest['canonical_staged_roadmap_sha256'] = digest(staged_roadmap)
env = os.environ.copy()
for key in ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE']:
    env.pop(key, None)
env['GIT_CEILING_DIRECTORIES'] = str(out)
patch = out / 'round1-latest-main.patch'
manifest['old_patch_sha256'] = digest(patch.read_bytes())
for name, directory in [('canonical', candidate), ('canonical_staged', staged)]:
    checked = subprocess.run(['git', 'apply', '--check', str(patch)], cwd=directory, env=env, capture_output=True, text=True)
    manifest[name + '_apply_check'] = {'exit_code': checked.returncode, 'output': checked.stdout + checked.stderr}
    if checked.returncode:
        (out / 'round2-portability.json').write_text(json.dumps(manifest, indent=2) + '\n')
        raise SystemExit('patch check failed: ' + name)
    subprocess.run(['git', 'apply', str(patch)], cwd=directory, env=env, check=True)
for f in changed[1:]:
    assert (candidate/f).read_bytes() == (root/f).read_bytes(), f
    assert (staged/f).read_bytes() == (root/f).read_bytes(), f
manifest['candidate_equals_assigned_trace'] = True
manifest['unchanged_trace_files'] = [f for f in trace_files if f not in changed]
for f in manifest['unchanged_trace_files']:
    assert (base/f).read_bytes() == (candidate/f).read_bytes(), f
    assert (base/f).read_bytes() == git('show', '2b321d2:' + f), f
parts = []
for f in changed:
    before = (base/f).read_text().splitlines(keepends=True)
    after = (candidate/f).read_text().splitlines(keepends=True)
    parts.append('diff --git a/' + f + ' b/' + f + '\n')
    parts.extend(difflib.unified_diff(before, after, fromfile='a/'+f, tofile='b/'+f))
fresh = out/'round2-c26c4a2.patch'
fresh.write_text(''.join(parts))
checked = subprocess.run(['git', 'apply', '--check', str(fresh)], cwd=base, env=env, capture_output=True, text=True)
manifest['fresh_patch_apply_check'] = {'exit_code': checked.returncode, 'output': checked.stdout+checked.stderr}
assert checked.returncode == 0
manifest['fresh_patch_sha256'] = digest(fresh.read_bytes())
manifest['source_guard_after'] = digest(git('diff', '--binary', 'HEAD'))
manifest['sources_unchanged'] = manifest['source_guard_after'] == guard
assert manifest['sources_unchanged']
manifest['source_sha256'] = {f: {'base': digest((base/f).read_bytes()), 'patched': digest((candidate/f).read_bytes())} for f in files}
(out/'round2-portability.json').write_text(json.dumps(manifest, indent=2)+'\n')
print(json.dumps({k:v for k,v in manifest.items() if k != 'source_sha256'}, indent=2))
