import hashlib,json,os,pathlib,resource,subprocess,time,traceback
root=pathlib.Path(__file__).resolve().parent.parent;out=root/'.codex-trace-checks';os.chdir(root)
def sha(data):return hashlib.sha256(data).hexdigest()
def guard():return sha(subprocess.check_output(['git','diff','--binary','HEAD']))
def write(name,data):
    p=out/name;t=p.with_suffix('.tmp');t.write_text(json.dumps(data,indent=2)+'\n');t.replace(p)
files=[out/'round3-scaled-controller.py',out/'round3-budget-probe.rs',*sorted((out/'round3-scaled-probe-source').glob('*.rs'))]
def inputs():return {str(p.relative_to(root)):sha(p.read_bytes()) for p in files}
start=guard();before=inputs();result={'guard_before':start,'input_hashes':before,'steps':[]};code=1
try:
    c=json.loads((out/'round2-current2-finished.json').read_text());assert c['exit_code']==0
    external={n:root/v['path'] for n,v in c['external_rlibs'].items()};external.update({n:root/c['built_rlibs'][n]['path'] for n in ['vectorcraft_geom','vectorcraft_pathops']})
    for n,v in c['external_rlibs'].items():assert sha(external[n].read_bytes())==v['sha256']
    deps=root/'target/debug/deps';lib=out/'libvectorcraft_trace_round3_scaled.rlib'
    args=['rustc','--edition=2024','--crate-name','vectorcraft_trace','--crate-type','rlib','-C','opt-level=1','-C','debug-assertions=yes','-C','debuginfo=0','-C','metadata=round3scaled','-L','dependency='+str(deps),'-L','dependency='+str(out)]
    for n in ['vectorcraft_geom','vectorcraft_pathops','image','serde','thiserror','kurbo']:args+=['--extern',n+'='+str(external[n])]
    args+=[str(out/'round3-scaled-probe-source/lib.rs'),'-o',str(lib)]
    commands=[('library',args)]
    for flavor,library in [('baseline',out/'libvectorcraft_trace_round3_readonly.rlib'),('scaled',lib)]:
        runner=out/('round3-budget-'+flavor)
        commands.append((flavor+'-compile',['rustc','--edition=2024','--crate-name','trace_round3_budget_'+flavor,'-C','opt-level=1','-C','debuginfo=0','-L','dependency='+str(deps),'-L','dependency='+str(out),'--extern','instrumented='+str(library),str(out/'round3-budget-probe.rs'),'-o',str(runner)]))
        if flavor=='baseline':commands.append(('baseline-zero',[str(runner),'baseline','zero']))
        else:commands.extend([('scaled-full',[str(runner),'scaled','full']),('scaled-zero',[str(runner),'scaled','zero'])])
    env=os.environ.copy();env.update(CARGO_BUILD_JOBS='1',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(root/'target'),MALLOC_ARENA_MAX='1')
    def tiny_limits():resource.setrlimit(resource.RLIMIT_AS,(4*(64<<20),4*(64<<20)));resource.setrlimit(resource.RLIMIT_CPU,(30,30))
    for name,cmd in commands:
        assert guard()==start and inputs()==before
        begin=time.time()
        with (out/('round3-scaled-'+name+'.log')).open('w') as log:
            child=subprocess.Popen(cmd,env=env,cwd=root,stdout=log,stderr=subprocess.STDOUT,preexec_fn=tiny_limits if name in ['baseline-zero','scaled-full','scaled-zero'] else None)
            write('round3-scaled-running.json',{'controller_pid':os.getpid(),'child_pid':child.pid,'step':name,'guard':start})
            try:status=child.wait(timeout=45)
            except subprocess.TimeoutExpired:child.kill();child.wait();raise RuntimeError('bounded timeout '+name)
        item={'step':name,'exit_code':status,'elapsed_seconds':time.time()-begin,'guard_after':guard(),'inputs_unchanged':inputs()==before};result['steps'].append(item);write('round3-scaled-'+name+'.exit.json',item)
        assert status==0,name
        assert guard()==start and inputs()==before
    code=0
except Exception as e:result['error']=str(e);traceback.print_exc()
finally:result.update(exit_code=code,guard_after=guard(),sources_unchanged=guard()==start,inputs_unchanged=inputs()==before);write('round3-scaled-finished.json',result)
raise SystemExit(code)
