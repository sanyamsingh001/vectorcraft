import hashlib,json,os,pathlib,subprocess,time,traceback
root=pathlib.Path(__file__).resolve().parent.parent;out=root/'.codex-trace-checks';os.chdir(root)
def sha(data):return hashlib.sha256(data).hexdigest()
def guard():return sha(subprocess.check_output(['git','diff','--binary','HEAD']))
def write(name,data):
    p=out/name;t=p.with_suffix('.tmp');t.write_text(json.dumps(data,indent=2)+'\n');t.replace(p)
def inputs():
    paths=[*sorted((out/'round3-frozen').rglob('*.rs')),out/'round3-frozen/ROADMAP.md',out/'round3-cumulative-c26.patch',out/'round3-resource-only.patch',out/'round3-verify-controller.py',out/'round2-adversarial.rs']
    return {str(p.relative_to(root)):sha(p.read_bytes()) for p in paths}
start=guard();before=inputs();result={'guard_before':start,'input_hashes':before,'steps':[]};code=1
try:
    checked=json.loads((out/'round2-current2-finished.json').read_text());assert checked['exit_code']==0
    external={n:root/v['path'] for n,v in checked['external_rlibs'].items()}
    external.update({n:root/checked['built_rlibs'][n]['path'] for n in ['vectorcraft_geom','vectorcraft_pathops','baseline']})
    for n,v in checked['external_rlibs'].items():assert sha(external[n].read_bytes())==v['sha256']
    deps=root/'target/debug/deps';lib=out/'libvectorcraft_trace_round3_guarded.rlib';external['candidate']=lib
    def build(crate,source,needed,target,test=False):
        args=['rustc','--edition=2024','--crate-name',crate,'-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-C','metadata=round3_'+target.name,'-L','dependency='+str(deps),'-L','dependency='+str(out)]
        args+=['--test'] if test else ['--crate-type','rlib']
        for n in needed:args+=['--extern',n+'='+str(external[n])]
        return args+[str(source),'-o',str(target)]
    src=out/'round3-frozen/crates/trace/src/lib.rs';needs=['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo']
    commands=[('library',build('vectorcraft_trace',src,needs,lib)),('suite-build',build('vectorcraft_trace',src,needs+['serde_json'],out/'round3-suite',True)),('suite',[str(out/'round3-suite'),'--test-threads=1']),('adversarial-build',build('trace_round3_adversarial',out/'round2-adversarial.rs',['baseline','candidate','serde_json'],out/'round3-adversarial',True)),('adversarial',[str(out/'round3-adversarial'),'--test-threads=1','--nocapture']),('clippy',['cargo','clippy','--locked','-p','vectorcraft-trace','--all-targets','--','-D','warnings']),('fmt',['cargo','fmt','-p','vectorcraft-trace','--','--check'])]
    env=os.environ.copy();env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'))
    for name,cmd in commands:
        assert guard()==start and inputs()==before
        begin=time.time()
        with (out/('round3-'+name+'.log')).open('w') as log:
            child=subprocess.Popen(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
            write('round3-running.json',{'controller_pid':os.getpid(),'child_pid':child.pid,'step':name,'guard':start})
            try:status=child.wait(timeout=120)
            except subprocess.TimeoutExpired:child.kill();child.wait();raise RuntimeError('bounded timeout '+name)
        item={'step':name,'exit_code':status,'elapsed_seconds':time.time()-begin,'guard_after':guard(),'inputs_unchanged':inputs()==before};result['steps'].append(item);write('round3-'+name+'.exit.json',item)
        assert status==0,name
        assert guard()==start and inputs()==before
    code=0
except Exception as e:result['error']=str(e);traceback.print_exc()
finally:result.update(exit_code=code,guard_after=guard(),sources_unchanged=guard()==start,inputs_unchanged=inputs()==before);write('round3-finished.json',result)
raise SystemExit(code)
