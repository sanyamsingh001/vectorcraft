use vectorcraft_trace::{Method,Mode,Raster,TraceParams,presets,trace};
fn main() {
 let args:Vec<String>=std::env::args().collect();
 if args.get(1).map(String::as_str)==Some("parity") {
  for fixture in 0..6 {
   let image=Raster::from_fn(64,64,|x,y| match fixture {
    0=>[80,120,160,255],
    1=>[0,0,0,0],
    2=>if (x<32)^(y<32) {[16,24,32,255]} else {[240,232,224,255]},
    3=>if x%13<4||y%17<5 {[80,120,160,255]} else {[0,0,0,0]},
    4=>[(x*4) as u8,(y*4) as u8,((x+y)*2) as u8,255],
    _=>if (x+2*y)%7==0 {[0,0,0,0]} else {let cell=(x/2).wrapping_mul(73856093)^(y/2).wrapping_mul(19349663);[cell as u8,(cell>>8)as u8,(cell>>16)as u8,255]},
   });
   let mut settings=presets();
   settings.push(("Noise exceeds area",TraceParams{mode:Mode::Color,colors:16,noise:u32::MAX,method:Method::Overlapping,..TraceParams::default()}));
   settings.push(("No denoise",TraceParams{mode:Mode::Color,colors:8,noise:0,method:Method::Abutting,..TraceParams::default()}));
   for (name,params) in settings { println!("fixture {fixture}, preset {name}: {:?}",trace(&image,&params)); }
  }
 } else {
  let photo=args.get(1).map(String::as_str)==Some("photo");
  let side=args.get(if photo {2} else {1}).and_then(|a|a.parse::<u32>().ok()).unwrap_or(4096);
  let noise=args.get(2).and_then(|a|a.parse::<u32>().ok()).unwrap_or(4);
  let image=Raster::from_fn(side,side,|x,y| if photo {[(x/8)as u8,(y/8)as u8,((x+y)/16)as u8,255]} else {[80,120,160,255]});
  let params=if photo {vectorcraft_trace::preset("High Fidelity Photo").unwrap()} else {TraceParams{mode:Mode::Color,colors:64,noise,method:Method::Overlapping,..TraceParams::default()}};
  let start=std::time::Instant::now();
  let result=trace(&image,&params);
  if !photo {assert_eq!(result.paths.len(),1);assert_eq!(result.anchor_count(),4);assert_eq!(result.paths[0].pixels,side as usize*side as usize);}
  println!("{side}x{side}, Noise={noise}, {} paths, {} anchors, trace_seconds={:.6}",result.paths.len(),result.anchor_count(),start.elapsed().as_secs_f64());
 }
}
