import hashlib, json, os, pathlib, subprocess, time, traceback
root=pathlib.Path(__file__).resolve().parent.parent
out=root/'.codex-trace-checks'
os.chdir(root)
def sha(data): return hashlib.sha256(data).hexdigest()
def guard(): return sha(subprocess.check_output(['git','diff','--binary','HEAD'],cwd=root))
def write(name,data):
    dest=out/name
    tmp=dest.with_suffix('.tmp')
    tmp.write_text(json.dumps(data,indent=2)+'\n')
    tmp.replace(dest)
def inputs():
    paths=[out/'round2-adversarial.rs',out/'round2-controller.py',out/'round2-c26c4a2.patch']
    for flavor in ['round2-c26-base','round2-c26-optimized']:
        paths.extend(sorted((out/flavor).rglob('*.rs')))
    paths.extend(out/p for p in ['libvectorcraft_trace_round1_upstream557.rlib','libvectorcraft_trace_round1_upstream557_optimized.rlib','round1-upstream-tests'])
    return {str(p.relative_to(root)):sha(p.read_bytes()) for p in paths}
start=guard(); input_before=inputs()
results={'source_guard_before':start,'input_hashes':input_before,'steps':[]}
code=1
try:
    assert json.loads((out/'round2-portability.json').read_text())['sources_unchanged']
    # Cached rlibs were built without cfg(test); test-only source differences do not enter them.
    for flavor,old in [('round2-c26-base',out/'round1-upstream/crates/trace/src'),('round2-c26-optimized',out/'round1-upstream-optimized')]:
        for p in sorted((out/flavor/'crates/trace/src').glob('*.rs')):
            if p.name!='tests.rs': assert p.read_bytes()==(old/p.name).read_bytes(),str(p)
    current=(out/'round2-c26-optimized/crates/trace/src/tests.rs').read_bytes()
    assert (out/'round1-upstream-tested/tests.rs').read_bytes().startswith(current)
    for p in sorted((out/'round2-c26-optimized/crates/trace/src').glob('*.rs')):
        if p.name!='tests.rs': assert p.read_bytes()==(out/'round1-upstream-tested'/p.name).read_bytes(),str(p)
    results['cached_production_sources_equal_c26']=True
    deps=root/'target/debug/deps'
    env=os.environ.copy()
    env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
    runner=out/'round2-adversarial-tests'
    commands=[('compile',['rustc','--test','--edition=2024','--crate-name','trace_round2_adversarial','-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-L','dependency='+str(deps),'--extern','baseline='+str(out/'libvectorcraft_trace_round1_upstream557.rlib'),'--extern','candidate='+str(out/'libvectorcraft_trace_round1_upstream557_optimized.rlib'),'--extern','serde_json='+str(deps/'libserde_json-160e40cebf7c5b27.rlib'),str(out/'round2-adversarial.rs'),'-o',str(runner)]),('adversarial',[str(runner),'--test-threads=1','--nocapture']),('composed-recovered',[str(out/'round1-upstream-tests'),'--test-threads=1'])]
    for name,cmd in commands:
        assert guard()==start and inputs()==input_before,'source changed before '+name
        started=time.time()
        with (out/('round2-'+name+'.log')).open('w') as log:
            proc=subprocess.Popen(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
            write('round2-running.json',{'controller_pid':os.getpid(),'child_pid':proc.pid,'step':name,'started':started,'source_guard':start})
            status=proc.wait()
        result={'step':name,'exit_code':status,'elapsed_seconds':time.time()-started,'source_guard_after':guard(),'inputs_unchanged':inputs()==input_before}
        write('round2-'+name+'.exit.json',result)
        results['steps'].append(result)
        assert status==0,'failed '+name
        assert guard()==start and inputs()==input_before,'source changed during '+name
    code=0
except Exception as e:
    results['error']=str(e)
    traceback.print_exc()
finally:
    results.update(source_guard_after=guard(),sources_unchanged=guard()==start,inputs_unchanged=inputs()==input_before,exit_code=code,finished=time.time())
    write('round2-finished.json',results)
raise SystemExit(code)
