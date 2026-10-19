use instrumented::{Raster,TraceParams,Mode,Method,trace_within,codex_scratch_probe};
fn main() {
    let workers=std::thread::available_parallelism().map_or(1,|n|n.get()).min(16);
    codex_scratch_probe::configure(workers);
    let image=Raster::from_fn(64,64,|x,_|{let c=x/4;[(c*16)as u8,((15-c)*16)as u8,(c*8)as u8,255]});
    let params=TraceParams {mode:Mode::Color,colors:16,noise:1,paths:100.0,corners:100.0,method:Method::Abutting,..Default::default()};
    let result=trace_within(&image,&params,64).unwrap();
    assert_eq!(result.palette.len(),16);
    assert_eq!(result.paths.len(),16);
    assert_eq!(result.anchor_count(),64);
    assert!(result.paths.iter().all(|p|p.path.anchor_count()==4&&p.pixels==256));
    let records=codex_scratch_probe::records();
    assert_eq!(records.len(),16);
    assert!(records.iter().all(|&(m,c,e)|m==4096&&c==4*4096&&e==65*65));
    let per_layer=6*4096+64+64+1;
    let simultaneous=workers*per_layer;
    println!("tiny={{\"width\":64,\"height\":64,\"layers\":16,\"paths\":16,\"anchors\":64,\"workers_observed\":{workers},\"fixed_scratch_per_layer_bytes\":{per_layer},\"controlled_simultaneous_fixed_scratch_bytes\":{simultaneous},\"decoded_rgba_bytes\":16384}}");
    // Original pattern scaled by dimensions only; never allocate this raster.
    let p=8192u64*8192;
    assert_eq!(p,instrumented::MAX_PIXELS);
    let fixed=6*p+8192+8192+1;
    let total=fixed*workers as u64;
    println!("model={{\"width\":8192,\"height\":8192,\"pixels\":{p},\"layers\":16,\"anchors\":64,\"fixed_scratch_per_layer_bytes\":{fixed},\"fixed_scratch_at_observed_worker_count_bytes\":{total},\"input_and_labels_bytes\":{},\"not_allocated\":true}}",6*p);
}
