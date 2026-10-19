import base64, hashlib, json, subprocess
from pathlib import Path

root = Path('/mnt/downloads/dev/artcraft/vectorcraft')
own = Path(__file__).resolve().parent
cli = root / 'target/debug/vectorcraft-cli'
assert hashlib.sha256(cli.read_bytes()).hexdigest() == '96ddec3800447387d468a518e1993f67eeeeac313ec154a1c81a48fda89570fe', 'CLI changed; re-establish source provenance before probing'
magic = b'\n\0VCBLOBS\0'

def run(commands):
    args = [str(cli), 'run']
    for command, params in commands:
        args += ['--cmd', command, '--params', json.dumps(params, separators=(',', ':'))]
    process = subprocess.run(args, cwd=own, capture_output=True, text=True, timeout=20)
    assert process.returncode == 0, process.stderr
    return [json.loads(line)['result'] for line in process.stdout.splitlines() if line.strip()]

def native_bytes(value):
    return base64.b64decode(value['dataBase64'])

source = own / 'negative-only-source.vectorcraft'
assert not source.exists()
source.write_bytes(native_bytes(run([
    ('file.new', {'width': 40, 'height': 30}),
    ('paint.setStroke', {'none': True}),
    ('shape.rectangle', {'x': 2, 'y': 3, 'width': 10, 'height': 8}),
    ('document.serialize', {'format': 'vectorcraft'})
])[-1]))
try:
    parent = native_bytes(run([
        ('file.new', {'width': 80, 'height': 60}),
        ('file.place', {'path': str(source), 'at': [20, 20]}),
        ('document.serialize', {'format': 'vectorcraft'})
    ])[-1])
finally:
    source.unlink()
setup = [('document.open', {'name': 'synthetic-native-parent.vectorcraft', 'dataBase64': base64.b64encode(parent).decode()}), ('document.json', {})]
for include in [False, True]:
    results = run(setup + [('document.serialize', {'format': 'vectorcraft', 'includeLinked': include, 'compress': False}), ('document.json', {})])
    saved = json.loads(native_bytes(results[2]).split(magic, 1)[0])
    kind = saved['document']['layers'][0]['kind']['children'][0]['kind']
    original = results[1]['layers'][0]['kind']['children'][0]['kind']
    print(json.dumps({'includeLinked': include, 'version': saved['version'], 'saved_type': kind['type'], 'link_present': 'link' in kind, 'warnings': results[2]['warnings'], 'source_preserved': results[1] == results[3]}), flush=True)
    assert saved['version'] == 4
    assert results[1] == results[3]
    assert kind == original, 'includeLinked:true discarded a missing PlacedDocument link in current native v4'
