use baseline as a;
use candidate as b;

fn fixture(id: usize) -> a::Raster {
    let (w,h) = match id {0 => (1,1),1 => (1,65),2 => (65,1),6 => (31,17),_ => (17,19)};
    a::Raster::from_fn(w,h,|x,y| match id {
        0 => [0,0,0,255],
        1|2 => if (x+y)%11==0 {[255,255,255,255]} else {[20,40,80,255]},
        3 => if x==w-1&&y==h-1 {[0,0,0,0]} else {[20,40,80,255]},
        4 => if (5..12).contains(&x)&&(5..14).contains(&y) {[0,0,0,0]} else {[20,40,80,255]},
        5 => [(x*15) as u8,(y*13) as u8,80,[0,1,127,128,254,255][((x+y)%6) as usize]],
        6 => {let i=x+y*w;[(i*37)as u8,(i*91)as u8,(i*53)as u8,255]},
        7 => [255,255,255,255],
        _ => [20,40,80,0],
    })
}
fn equivalent(img:&a::Raster, p:&a::TraceParams) -> (b::Raster,b::TraceParams) {
    let image=b::Raster {width:img.width,height:img.height,rgba:img.rgba.clone()};
    let params=serde_json::from_str(&serde_json::to_string(p).unwrap()).unwrap();
    (image,params)
}
#[test]
fn adversarial_complete_trace_outputs_match_c26_baseline() {
    let mut checked=0;
    for id in 0..9 {
        let img=fixture(id);
        for mode in [a::Mode::BlackAndWhite,a::Mode::Grayscale,a::Mode::Color] {
            for method in [a::Method::Abutting,a::Method::Overlapping] {
                for noise in [0,2,img.width*img.height+1,u32::MAX] {
                    for ignore_white in [false,true] {
                        let p=a::TraceParams {mode,method,noise,ignore_white,colors:256,paths:100.0,corners:100.0,..Default::default()};
                        let (other,q)=equivalent(&img,&p);
                        assert_eq!(format!("{:?}",a::trace(&img,&p)),format!("{:?}",b::trace(&other,&q)),"fixture {id}, {p:?}");
                        checked+=1;
                    }
                }
            }
        }
    }
    assert_eq!(checked,432);
    println!("{checked} complete outputs exactly match: skinny, near-uniform, transparent holes, alpha 127/128, 256 colors, white filtering, Noise 0/2/area+1/MAX");
}
#[test]
fn adversarial_anchor_budgets_preserve_all_results_and_errors() {
    let mut checked=0;
    for id in [0,1,3,4,7,8] {
        let img=fixture(id);
        for method in [a::Method::Abutting,a::Method::Overlapping] {
            for ignore_white in [false,true] {
                let p=a::TraceParams {mode:a::Mode::Color,method,ignore_white,colors:2,noise:2,..Default::default()};
                let (other,q)=equivalent(&img,&p);
                let full=a::trace(&img,&p);
                let full_other=b::trace(&other,&q);
                let anchors=full.anchor_count();
                assert_eq!(anchors,full_other.anchor_count());
                for budget in [0,anchors.saturating_sub(1),anchors,anchors+1] {
                    let left=a::trace_within(&img,&p,budget);
                    let right=b::trace_within(&other,&q,budget);
                    assert_eq!(format!("{left:?}"),format!("{right:?}"),"fixture {id}, budget {budget}, {p:?}");
                    if budget<anchors {
                        assert!(matches!(right,Err(b::TraceError::TooComplex {max}) if max==budget));
                    } else {
                        assert_eq!(right.unwrap(),full_other);
                    }
                    checked+=1;
                }
            }
        }
    }
    assert_eq!(checked,96);
    println!("{checked} paired anchor-budget checks preserve zero/exact/one-under/one-over behavior, including empty output");
}
