import hashlib, json, os, pathlib, subprocess, time
root = pathlib.Path(__file__).resolve().parent.parent
out = root / '.codex-trace-checks'
os.chdir(root)
def guard():
    return hashlib.sha256(subprocess.check_output(['git', 'diff', '--binary', 'HEAD'])).hexdigest()
def write(path, data):
    tmp = path.with_suffix('.tmp')
    tmp.write_text(json.dumps(data, indent=2) + '\n')
    tmp.replace(path)
env = os.environ.copy()
env.update(CARGO_BUILD_JOBS='1', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_TARGET_DIR=str(root / 'target'))
start_guard = guard()
write(out/'running.json', {'controller_pid':os.getpid(), 'guard':start_guard, 'started':time.time()})
commands = [
    ('trace-tests', ['cargo', 'test', '-p', 'vectorcraft-trace', '--lib']),
    ('trace-performance', ['cargo', 'test', '-p', 'vectorcraft-trace', '--lib', 'timing_2048x2048_high_fidelity', '--', '--ignored', '--nocapture']),
    ('trace-integration', ['cargo', 'test', '-p', 'vectorcraft-engine', '-p', 'vectorcraft-ui-egui', '--lib', 'image_trace']),
    ('trace-clippy', ['cargo', 'clippy', '-p', 'vectorcraft-trace', '--all-targets', '--', '-D', 'warnings']),
]
for name, cmd in commands:
    saved = out/(name+'.exit.json')
    if saved.exists():
        result = json.loads(saved.read_text())
        if result.get('exit_code') == 0 and result.get('guard_after') == start_guard and result.get('command') == cmd:
            continue
    started = time.time()
    with (out/(name+'.log')).open('w') as log:
        try:
            proc = subprocess.Popen(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
        except OSError as error:
            write(out/(name+'.exit.json'), {'command':cmd, 'exit_code':127, 'error':str(error), 'guard_before':start_guard, 'guard_after':guard()})
            break
        write(out/'running.json', {'controller_pid':os.getpid(), 'child_pid':proc.pid, 'command':cmd, 'guard':start_guard, 'started':started})
        _, status, usage = os.wait4(proc.pid, 0)
        code = os.waitstatus_to_exitcode(status)
        proc.returncode = code
    end_guard = guard()
    write(out/(name+'.exit.json'), {'command':cmd, 'exit_code':code, 'elapsed_seconds':time.time()-started, 'max_rss_kb':usage.ru_maxrss, 'guard_before':start_guard, 'guard_after':end_guard, 'sources_unchanged':start_guard==end_guard})
    if code != 0 or start_guard != end_guard:
        break
write(out/'finished.json', {'finished':time.time(), 'guard':guard(), 'sources_unchanged':guard()==start_guard})
