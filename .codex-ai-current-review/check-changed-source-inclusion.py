import base64
import hashlib
import json
import re
import select
import subprocess
import time
import zlib
from pathlib import Path

ROOT = Path('/mnt/downloads/dev/artcraft/vectorcraft')
OWN = Path(__file__).resolve().parent
FIXTURES = OWN / 'changed-source-inclusion-fixtures'
CLI = ROOT / 'target/debug/vectorcraft-cli'
EXPECTED_CLI = '45f9a68caf6f40192365c32754af1b985b698b8c81c87fb8c4b5254a0492fb37'
EXPECTED_GUARD = '02d306cd6779d2e1f58e7f93d83b2c6b3cdef584f112d44d0cf9296064eb61d8'
MAGIC = b'\n\0VCBLOBS\0'


def digest(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def guard():
    return hashlib.sha256(subprocess.check_output(['git', 'diff', '--cached', '--binary'], cwd=ROOT)).hexdigest()


def run(commands):
    args = [str(CLI), 'run']
    for cmd, params in commands:
        args += ['--cmd', cmd, '--params', json.dumps(params, separators=(',', ':'))]
    p = subprocess.run(args, cwd=OWN, capture_output=True, text=True, timeout=15)
    assert p.returncode == 0, p.stderr
    return [json.loads(line)['result'] for line in p.stdout.splitlines() if line]


def data(result):
    return base64.b64decode(result['dataBase64'])


def native(raw):
    if raw.startswith(b'\x1f\x8b'):
        import gzip
        raw = gzip.decompress(raw)
    head, sep, tail = raw.partition(MAGIC)
    v = json.loads(head)
    return v, tail if sep else b''


def attachment(pdf):
    found = []
    for match in re.finditer(rb'\d+ 0 obj(.*?)endobj', pdf, re.S):
        obj = match[1]
        if b'/Type/EmbeddedFile' not in obj and b'/Type /EmbeddedFile' not in obj:
            continue
        header, sep, body = obj.partition(b'stream\n')
        assert sep, 'original generated PDF attachment has no stream'
        stream = body.rsplit(b'\nendstream', 1)[0]
        if b'/FlateDecode' in header:
            stream = zlib.decompress(stream)
        found.append(stream)
    assert len(found) == 1, len(found)
    return found[0]


def image_bytes(image, tail):
    if 'blob' in image:
        offset, length = image['blob']
        return tail[offset:offset + length]
    return base64.b64decode(image['data'])


def placed(doc):
    return doc['layers'][0]['kind']['children'][0]['kind']


class Session:
    def __init__(self):
        self.proc = subprocess.Popen([str(CLI), 'mcp', '--headless'], cwd=OWN,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, text=True, bufsize=1)
        self.id = 0
        self.rpc('initialize', {'protocolVersion': '2024-11-05', 'capabilities': {},
                               'clientInfo': {'name': 'original-fixture-review', 'version': '1'}})

    def rpc(self, method, params):
        self.id += 1
        self.proc.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': self.id,
                                         'method': method, 'params': params}) + '\n')
        self.proc.stdin.flush()
        assert select.select([self.proc.stdout], [], [], 15)[0], 'bounded local CLI response timeout'
        reply = json.loads(self.proc.stdout.readline())
        assert reply['id'] == self.id and 'error' not in reply, reply
        return reply['result']

    def command(self, command, params=None):
        result = self.rpc('tools/call', {'name': 'run_command',
                                       'arguments': {'command': command, 'params': params or {}}})
        assert not result.get('isError'), result
        return json.loads(result['content'][0]['text'])

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(timeout=10)
        assert self.proc.returncode == 0, self.proc.stderr.read()
        self.proc.stdout.close()
        self.proc.stderr.close()


assert digest(CLI) == EXPECTED_CLI, 'CLI changed; re-establish provenance'
assert guard() == EXPECTED_GUARD, 'root gate source changed'
gate = json.loads((ROOT / '.git/codex/gate-files-e037-r3/source.json').read_text())
assert gate['diff'] == EXPECTED_GUARD
FIXTURES.mkdir(exist_ok=True)
source = FIXTURES / 'original-small-source.vectorcraft'
source_commands = [('file.new', {'width': 40, 'height': 30}),
                   ('paint.setStroke', {'none': True}),
                   ('paint.setFill', {'color': '#e61414'}),
                   ('shape.rectangle', {'x': 2, 'y': 3, 'width': 10, 'height': 8})]
old = data(run(source_commands + [('document.serialize', {'format': 'vectorcraft', 'compress': False})])[-1])
new = data(run(source_commands[:2] + [('paint.setFill', {'color': '#1414e6'}),
                  ('shape.rectangle', {'x': 2, 'y': 3, 'width': 12, 'height': 8}),
                  ('shape.rectangle', {'x': 30, 'y': 20, 'width': 2, 'height': 2}),
                  ('document.serialize', {'format': 'vectorcraft', 'compress': False})])[-1])
assert old != new and len(old) != len(new)
source.write_bytes(old)
parent = data(run([('file.new', {'width': 80, 'height': 60}),
                   ('file.place', {'path': str(source), 'at': [20, 20]}),
                   ('document.serialize', {'format': 'vectorcraft', 'compress': False})])[-1])
parent_native, _ = native(parent)
old_key = placed(parent_native['document'])['key']
assert parent_native['images'][old_key]['proxy'] is True
(FIXTURES / 'original-parent-proxy.vectorcraft').write_bytes(parent)
(FIXTURES / 'old-source-bytes.vectorcraft').write_bytes(old)
(FIXTURES / 'new-source-bytes.vectorcraft').write_bytes(new)
rows = []
scenarios = ['unchanged', 'changed_cold', 'changed_cached_old', 'changed_then_update']
for scenario in scenarios:
    source.write_bytes(old)
    session = Session()
    try:
        session.command('document.open', {'name': 'original-parent-proxy.vectorcraft',
                                         'dataBase64': base64.b64encode(parent).decode()})
        before = session.command('document.json')
        initial = placed(before)
        assert initial['type'] == 'placeddocument' and initial['key'] == old_key, initial
        if scenario == 'changed_cached_old':
            warm, tail = native(data(session.command('document.serialize',
                         {'format': 'vectorcraft', 'includeLinked': True, 'compress': False})))
            assert image_bytes(warm['images'][old_key], tail) == old
        if scenario != 'unchanged':
            source.write_bytes(new)
            check = session.command('links.check')
            assert check['modified'] == 1, check
        if scenario == 'changed_then_update':
            update = session.command('links.update')
            assert len(update['updated']) == 1, update
        before = session.command('document.json')
        expected = placed(before)
        assert (expected['key'] != old_key) == (scenario == 'changed_then_update')
        for cmd in ['document.serialize', 'document.export', 'document.save']:
            for fmt in ['vectorcraft', 'ai']:
                for include in [False, True]:
                    encoded = session.command(cmd, {'format': fmt, 'includeLinked': include,
                                                    'compress': False, 'pdfCompatible': False,
                                                    'modified': None})
                    raw = data(encoded)
                    payload = attachment(raw) if fmt == 'ai' else raw
                    saved, tail = native(payload)
                    kind = placed(saved['document'])
                    assert saved['version'] == 4
                    assert kind == expected, (scenario, cmd, fmt, include, kind, expected)
                    assert expected['key'] in saved['images']
                    blob = saved['images'][expected['key']]
                    content = image_bytes(blob, tail)
                    is_proxy = blob.get('proxy', False)
                    should_proxy = not include or scenario == 'changed_cold'
                    assert is_proxy == should_proxy, (scenario, cmd, fmt, include, blob)
                    expected_bytes = new if scenario == 'changed_then_update' else old
                    if not should_proxy:
                        assert content == expected_bytes, (scenario, cmd, fmt, include, len(content))
                    else:
                        assert content != new and content != old
                    assert session.command('document.json') == before, 'encoding mutated original source document'
                    rows.append({'scenario': scenario, 'command': cmd, 'format': fmt,
                                 'includeLinked': include, 'placed_metadata_preserved': True,
                                 'key': kind['key'], 'proxy': is_proxy,
                                 'embedded_bytes': len(content), 'exact_source': None if should_proxy else ('new' if scenario == 'changed_then_update' else 'old'),
                                 'warnings': encoded.get('warnings', [])})
    finally:
        session.close()
assert digest(CLI) == EXPECTED_CLI
assert guard() == EXPECTED_GUARD
paths = ['crates/engine/src/cmd/place/document.rs', 'crates/doc/src/placed_document.rs',
         'crates/engine/src/cmd/links.rs', 'crates/engine/src/cmd/fileio/native.rs',
         'crates/engine/src/cmd/fileio/save.rs', 'crates/engine/src/cmd/fileio/encode.rs',
         'crates/format/src/lib.rs']
report = {'gate': 'gate-files-e037-r3', 'root_guard': EXPECTED_GUARD,
          'head': gate['head'], 'cli_sha256': EXPECTED_CLI,
          'cli_mtime': CLI.stat().st_mtime, 'cli_provenance': 'Executable and Cargo fingerprint emitted during frozen gate-r3 root build; gate ci.log records CLI compilation. Hash and root guard verified before and after probe. Gate-wide success is not inferred.',
          'source_sha256': {p: digest(ROOT / p) for p in paths},
          'source_sizes': {'old': len(old), 'new': len(new)},
          'source_sha256_fixture': {'old': hashlib.sha256(old).hexdigest(), 'new': hashlib.sha256(new).hexdigest()},
          'rows': rows, 'passed': len(rows), 'findings': [], 'cargo_runs': 0,
          'root_source_edits': 0, 'network_actions': 0, 'final_clean_round_claim': False,
          'limits': 'Small original single-level source and proxy parent, default Ask When Modified then explicit Update Links; optional preview/PDF content and hashless legacy input are outside this probe.'}
(OWN / 'changed-source-inclusion-results.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'passed': len(rows), 'scenarios': scenarios, 'findings': [], 'source_sizes': report['source_sizes'], 'root_guard_unchanged': True, 'cli_hash_unchanged': True}), flush=True)
