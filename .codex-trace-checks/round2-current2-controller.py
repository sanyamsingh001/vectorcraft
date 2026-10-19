import hashlib,json,os,pathlib,subprocess,time,traceback
root=pathlib.Path(__file__).resolve().parent.parent
out=root/'.codex-trace-checks'
os.chdir(root)
def sha(data):return hashlib.sha256(data).hexdigest()
def guard():return sha(subprocess.check_output(['git','diff','--binary','HEAD'],cwd=root))
def write(name,data):
    dest=out/name;tmp=dest.with_suffix('.tmp');tmp.write_text(json.dumps(data,indent=2)+'\n');tmp.replace(dest)
def source_hashes():
    files=[out/'round2-current2-controller.py',out/'round2-adversarial.rs',out/'round2-c26c4a2.patch']
    for folder in ['round2-current-dependencies','round2-c26-base','round2-c26-optimized']:
        files.extend(sorted((out/folder).rglob('*.rs')))
    return {str(f.relative_to(root)):sha(f.read_bytes()) for f in files}
start=guard();before=source_hashes();results={'source_guard_before':start,'input_hashes':before,'steps':[]};code=1
try:
    deps=root/'target/debug/deps'
    extern={}
    for name in ['kurbo','serde','thiserror','linesweeper','image']:
        extern[name]=max(deps.glob('lib'+name+'-*.rlib'),key=lambda f:f.stat().st_mtime_ns)
    extern['kurbo']=deps/'libkurbo-3e0ca2f0ffd584e6.rlib'
    extern['serde_json']=deps/'libserde_json-160e40cebf7c5b27.rlib'
    results['external_rlibs']={n:{'path':str(p.relative_to(root)),'sha256':sha(p.read_bytes())} for n,p in extern.items()}
    extern['vectorcraft_geom']=out/'libvectorcraft_geom_round2_current2.rlib'
    extern['vectorcraft_pathops']=out/'libvectorcraft_pathops_round2_current2.rlib'
    extern['baseline']=out/'libvectorcraft_trace_round2_current2_baseline.rlib'
    extern['candidate']=out/'libvectorcraft_trace_round2_current2_candidate.rlib'
    def build(name,source,needed,target,test=False):
        args=['rustc','--edition=2024','--crate-name',name,'-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-C','metadata=round2current_'+name+('_test' if test else str(target.name)),'-L','dependency='+str(deps),'-L','dependency='+str(out)]
        args+=['--test'] if test else ['--crate-type','rlib']
        for dep in needed:args+=['--extern',dep+'='+str(extern[dep])]
        return args+[str(source),'-o',str(target)]
    trace_needed=['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo']
    commands=[
      ('geom-compile',build('vectorcraft_geom',out/'round2-current-dependencies/crates/geom/src/lib.rs',['kurbo','serde','thiserror'],extern['vectorcraft_geom'])),
      ('pathops-compile',build('vectorcraft_pathops',out/'round2-current-dependencies/crates/pathops/src/lib.rs',['vectorcraft_geom','kurbo','thiserror','linesweeper'],extern['vectorcraft_pathops'])),
      ('baseline-compile',build('vectorcraft_trace',out/'round2-c26-base/crates/trace/src/lib.rs',trace_needed,extern['baseline'])),
      ('candidate-compile',build('vectorcraft_trace',out/'round2-c26-optimized/crates/trace/src/lib.rs',trace_needed,extern['candidate'])),
      ('suite-compile',build('vectorcraft_trace',out/'round2-c26-optimized/crates/trace/src/lib.rs',trace_needed+['serde_json'],out/'round2-current2-suite',test=True)),
      ('suite',[str(out/'round2-current2-suite'),'--test-threads=1']),
      ('probe-compile',build('trace_round2_current2_adversarial',out/'round2-adversarial.rs',['baseline','candidate','serde_json'],out/'round2-current2-adversarial',test=True)),
      ('adversarial',[str(out/'round2-current2-adversarial'),'--test-threads=1','--nocapture'])
    ]
    env=os.environ.copy();env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'),CARGO_PKG_VERSION='0.7.0')
    for name,cmd in commands:
        assert guard()==start and source_hashes()==before,'changed before '+name
        started=time.time()
        with (out/('round2-current2-'+name+'.log')).open('w') as log:
            proc=subprocess.Popen(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
            write('round2-current2-running.json',{'controller_pid':os.getpid(),'child_pid':proc.pid,'step':name,'started':started,'source_guard':start})
            status=proc.wait()
        result={'step':name,'exit_code':status,'elapsed_seconds':time.time()-started,'source_guard_after':guard(),'inputs_unchanged':source_hashes()==before}
        write('round2-current2-'+name+'.exit.json',result);results['steps'].append(result)
        assert status==0,'failed '+name
        assert guard()==start and source_hashes()==before,'changed during '+name
    results['built_rlibs']={n:{'path':str(extern[n].relative_to(root)),'sha256':sha(extern[n].read_bytes())} for n in ['vectorcraft_geom','vectorcraft_pathops','baseline','candidate']}
    code=0
except Exception as e:
    results['error']=str(e);traceback.print_exc()
finally:
    results.update(source_guard_after=guard(),sources_unchanged=guard()==start,inputs_unchanged=source_hashes()==before,exit_code=code,finished=time.time());write('round2-current2-finished.json',results)
raise SystemExit(code)
