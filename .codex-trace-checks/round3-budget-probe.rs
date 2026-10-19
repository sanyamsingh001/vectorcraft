use instrumented::{Raster,TraceParams,Mode,Method,trace_within,codex_scratch_probe,TraceError};
fn main() {
    let scaled=std::env::args().nth(1).as_deref()==Some("scaled");
    let zero=std::env::args().nth(2).as_deref()==Some("zero");
    let workers=if scaled {1} else {std::thread::available_parallelism().map_or(1,|n|n.get()).min(16)};
    codex_scratch_probe::configure(workers);
    let image=Raster::from_fn(64,64,|x,_|{let c=x/4;[(c*16)as u8,((15-c)*16)as u8,(c*8)as u8,255]});
    let params=TraceParams {mode:Mode::Color,colors:16,noise:1,paths:100.0,corners:100.0,method:Method::Abutting,..Default::default()};
    let budget=if zero {0} else {64};
    let result=trace_within(&image,&params,budget);
    if zero {assert!(matches!(result,Err(TraceError::TooComplex {max:0})));}
    else {let r=result.unwrap();assert_eq!(r.paths.len(),16);assert_eq!(r.anchor_count(),64);assert!(r.paths.iter().all(|p|p.path.anchor_count()==4&&p.pixels==256));}
    let records=codex_scratch_probe::records();
    let expected=if scaled&&zero {1} else {16};
    assert_eq!(records.len(),expected);
    assert!(records.iter().all(|&(m,c,e)|m==4096&&c==16384&&e==4225));
    let simultaneous=workers*24705;
    println!("{{\"scaled_existing_pixel_budget\":{scaled},\"anchor_budget\":{budget},\"nonuniform_layer_scratch_allocations\":{},\"fixed_scratch_bytes_per_layer\":24705,\"controlled_simultaneous_fixed_scratch_bytes\":{simultaneous},\"only_64x64_input\":true}}",records.len());
}
