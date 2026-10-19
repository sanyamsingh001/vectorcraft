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



CANDIDATE = ROOT / '.git/codex/worktrees/files-current'
EXPECTED_CANDIDATE = '22785ca7bb8e2d5bbd729957d576f815dfc038a8525670496f21c66690ce618d'
EXPECTED_GUARD = EXPECTED_CANDIDATE

def candidate_guard():
    return hashlib.sha256(subprocess.check_output(['git', 'diff', '--cached', '--binary'], cwd=CANDIDATE)).hexdigest()

assert digest(CLI) == EXPECTED_CLI
EXPECTED_GUARD = guard()
EXPECTED_CANDIDATE = candidate_guard()
relevant = ['crates/engine/src/cmd/fileio/save.rs', 'crates/engine/src/cmd/fileio/native.rs',
            'crates/engine/src/cmd/fileio/mod.rs', 'crates/engine/src/cmd/fileio/export.rs',
            'crates/engine/src/cmd/place/document.rs', 'crates/engine/src/cmd/links.rs',
            'crates/doc/src/placed_document.rs', 'crates/format/src/lib.rs']
for rel in relevant:
    assert digest(ROOT / rel) == digest(CANDIDATE / rel), rel
scope_before = {rel: digest(CANDIDATE / rel) for rel in relevant}
assert scope_before['crates/engine/src/cmd/fileio/mod.rs'] == 'ed691fc213a36d2791358a7338bd6ed45295ceb1b8dad42f9adfb087aed77aed'
# Prove the cached r3 native/AI scope still matches the current r5 candidate.
prior = json.loads((OWN / 'changed-source-inclusion-results.json').read_text())
assert prior['cli_sha256'] == EXPECTED_CLI
for rel in relevant:
    if rel in prior['source_sha256']:
        assert digest(CANDIDATE / rel) == prior['source_sha256'][rel], rel
encode = (CANDIDATE / 'crates/engine/src/cmd/fileio/encode.rs').read_text()
old_encode = encode.replace('"vectorcraft" | "template" | "ai" | "svg" | "svgz")', '"vectorcraft" | "template" | "ai")', 1)
old_encode = old_encode.replace('// Formats with editing data prepare their drawn output separately from their original\n    // document. Other formats only need the placed files\' art.', '// Placed documents output their files\' art (a native file keeps what it keeps: see\n    // `native::encode`).')
assert hashlib.sha256(old_encode.encode()).hexdigest() == prior['source_sha256']['crates/engine/src/cmd/fileio/encode.rs']
fixtures = OWN / 'public-command-residual-fixtures'
fixtures.mkdir(exist_ok=True)
source = fixtures / 'small-linked.vectorcraft'
source_bytes = (OWN / 'changed-source-inclusion-fixtures/old-source-bytes.vectorcraft').read_bytes()
source.write_bytes(source_bytes)
created = run([('file.new', {'width': 80, 'height': 60, 'artboards': 2}),
               ('file.place', {'path': str(source), 'crop': 'bounding', 'at': [30, 20]}),
               ('object.transform', {'matrix': [1.15, 0.25, -0.35, 0.8, 7, 3]}),
               ('document.serialize', {'format': 'vectorcraft', 'compress': False})])
parent = data(created[-1])
head, _ = native(parent)
original = placed(head['document'])
assert original['type'] == 'placeddocument' and original['bounding'] is True
assert original['xf'][:4] != [1, 0, 0, 1]
assert head['images'][original['key']]['proxy'] is True
(fixtures / 'transformed-parent-proxy.vectorcraft').write_bytes(parent)
aliases = {'vectorcraft': 'vectorcraft', '.vectorcraft': 'vectorcraft', 'VECTORCRAFT': 'vectorcraft',
           'drawcraft': 'vectorcraft', '.DRAWCRAFT': 'vectorcraft',
           'template': 'template', 'vctemplate': 'template', '.VCTEMPLATE': 'template',
           'ai': 'ai', '.ai': 'ai', 'AI': 'ai', '.AI': 'ai'}
rows = []
negatives = []
for missing in [False, True]:
    if missing:
        source.unlink()
    session = Session()
    try:
        session.command('document.open', {'name': 'transformed-parent.vectorcraft', 'dataBase64': base64.b64encode(parent).decode()})
        before = session.command('document.json')
        expected = placed(before)
        assert expected == original, (missing, expected, original)
        options = session.command('file.formatOptions', {'format': '.VECTORCRAFT'})
        assert options['options']['version']['value'] == 4, options
        check = session.command('links.check')
        assert check['missing'] == int(missing), check
        requests = []
        for command in ['document.serialize', 'document.export', 'document.save']:
            for fmt in ['vectorcraft', 'template', 'ai']:
                for include in [False, True]:
                    requests.append((command, fmt, fmt, include, 'matrix'))
        for alias, canonical in aliases.items():
            requests.append(('document.save', alias, canonical, True, 'public-alias'))
        for command, alias, canonical, include, category in requests:
            params = {'format': alias, 'includeLinked': include, 'compress': False,
                      'pdfCompatible': False, 'modified': None}
            result = session.command(command, params)
            if 'format' in result:
                assert result['format'] == canonical, (alias, result['format'])
            raw = data(result)
            assert raw.startswith(b'%PDF') == (canonical == 'ai')
            saved, tail = native(attachment(raw) if canonical == 'ai' else raw)
            assert saved['version'] == 4
            assert placed(saved['document']) == expected, (missing, command, alias, 'transform/metadata changed')
            key = expected['key']
            blob = saved['images'][key]
            proxy = blob.get('proxy', False)
            assert proxy == (missing or not include), (missing, command, alias, include, blob)
            if not proxy:
                assert image_bytes(blob, tail) == source_bytes, (missing, command, alias)
            assert session.command('document.json') == before, (command, alias, 'source changed')
            rows.append({'missing': missing, 'command': command, 'alias': alias, 'format': canonical,
                         'includeLinked': include, 'proxy': proxy, 'version': saved['version'],
                         'category': category, 'transform_and_metadata_preserved': True,
                         'exact_source_bytes': not proxy})
        for command in ['document.serialize', 'document.save']:
            result = session.command(command, {'modified': None})
            saved, _ = native(data(result))
            assert saved['version'] == 4 and placed(saved['document']) == expected
            rows.append({'missing': missing, 'command': command, 'category': 'implicit-native-v4', 'version': 4})
        if not missing:
            cases=[]
            for key in ['includeLinked', 'embedProfiles', 'compress', 'pdfCompatible']:
                for subset in [False, True]:
                    params={'format': '.AI', key: 'yes'}
                    if subset: params['range']='2'
                    cases.append(('document.export', params))
                cases.append(('document.save', {'format': 'AI', 'options': {key: 'yes'}}))
            cases.append(('document.save', {'format': '.VECTORCRAFT', 'version': 5}))
            for command, params in cases:
                result = session.rpc('tools/call', {'name': 'run_command', 'arguments': {'command': command, 'params': params}})
                assert result['isError'], (command, params, result)
                assert session.command('document.json') == before
                negatives.append({'command': command, 'params': params, 'error': result['content'][0]['text'], 'source_unchanged': True})
    finally:
        session.close()
assert digest(CLI) == EXPECTED_CLI
end_root_guard = guard()
end_candidate_guard = candidate_guard()
assert all(digest(CANDIDATE / rel) == value and digest(ROOT / rel) == value for rel,value in scope_before.items())
report={'candidate': str(CANDIDATE), 'candidate_guard': EXPECTED_CANDIDATE, 'end_candidate_guard': end_candidate_guard,
        'root_gate': 'gate-files-e037-r5', 'root_guard': EXPECTED_GUARD, 'end_root_guard': end_root_guard, 'cli_sha256': EXPECTED_CLI,
        'cli_scope_match': 'Cached r3 executable; save/native/format/placed/export scope unchanged. Reversing only the SVG/SVGZ exemption and its comment reproduces the prior recorded encode.rs SHA; native/template/AI take the identical exempt branch. SVG correction not runtime-tested.',
        'rows': rows, 'expected_errors': negatives, 'success_cases': len(rows), 'negative_controls': len(negatives),
        'source_sha256': {rel: digest(CANDIDATE / rel) for rel in relevant},
        'production_findings': [], 'cargo_runs': 0, 'root_or_candidate_source_edits': 0,
        'new_svg_fix_runtime': 'Not exercised: cached executable lacks the candidate SVG boundary correction.',
        'final_clean_round_claim': False}
(OWN / 'public-command-residual-results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'success_cases':len(rows),'expected_errors':len(negatives),'production_findings':[],'guards_unchanged':end_root_guard == EXPECTED_GUARD and end_candidate_guard == EXPECTED_CANDIDATE, 'scope_hashes_unchanged':True}), flush=True)
