//! Temporary public-API indexed PNG benchmark. No implementation inspection.
#![forbid(unsafe_code)]
use raipixel::{Image, Limits, Work};
use std::{collections::HashMap, hint::black_box, io::Cursor, time::{Duration,Instant}};

type DynError = Box<dyn std::error::Error>;

fn packed(image: &Image) -> Result<Option<(Vec<u8>,Vec<u8>,Vec<u8>,png::BitDepth)>,DynError> {
    let [w,h] = image.size();
    let mut map=HashMap::<[u8;4],u8>::new();
    let mut colors=Vec::<[u8;4]>::new();
    for &sample in image.pixels() {
        if let std::collections::hash_map::Entry::Vacant(slot)=map.entry(sample) {
            if colors.len()==256 {return Ok(None);}
            slot.insert(colors.len() as u8);
            colors.push(sample);
        }
    }
    let bitdepth: usize = match colors.len(){0..=2=>1,3..=4=>2,5..=16=>4,_=>8};
    let depth = match bitdepth {
        1=>png::BitDepth::One,2=>png::BitDepth::Two,
        4=>png::BitDepth::Four,8=>png::BitDepth::Eight,_=>unreachable!()
    };
    let mut palette=Vec::<u8>::new();
    let mut alpha=Vec::<u8>::new();
    for color in colors {
        palette.extend_from_slice(&color[..3]);
        alpha.push(color[3]);
    }
    let width=w as usize;
    let bytes_per_row=(width*bitdepth).div_ceil(8);
    let mut indices=vec![0u8;bytes_per_row*h as usize];
    let per_byte=8/bitdepth;
    for y in 0..h as usize {
        for x in 0..width {
            let pixel=image.pixels()[y*width+x];
            let index=*map.get(&pixel).ok_or("missing indexed sample")?;
            indices[y*bytes_per_row+x/per_byte] |=
                index << (8-bitdepth*(x%per_byte+1));
        }
    }
    Ok(Some((palette,alpha,indices,depth)))
}

fn encode_ref(image:&Image,fast:bool)->Result<Option<Vec<u8>>,DynError>{
    let Some((palette,alpha,indices,depth))=packed(image)? else{return Ok(None)};
    let [w,h]=image.size();
    let mut out=Vec::new();
    {
        let mut encoder=png::Encoder::new(&mut out,w,h);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(depth);
        encoder.set_palette(palette);
        if alpha.iter().any(|&a|a!=255){encoder.set_trns(alpha);}
        if fast {encoder.set_compression(png::Compression::Fast);}
        let mut writer=encoder.write_header()?;
        writer.write_image_data(&indices)?;
        writer.finish()?;
    }
    Ok(Some(out))
}

fn reference_rgba(bytes:&[u8])->Result<(u32,u32,Vec<u8>),DynError>{
    let mut decoder=png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    decoder.ignore_checksums(false);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader=decoder.read_info()?;
    let mut buffer=vec![0u8;reader.output_buffer_size().ok_or("reference decode buffer")?];
    let info=reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    reader.finish()?;
    let rgba=match info.color_type {
        png::ColorType::Rgba=>buffer,
        png::ColorType::Rgb=>buffer.chunks_exact(3)
            .flat_map(|rgb|[rgb[0],rgb[1],rgb[2],255]).collect(),
        _=>return Err("reference decoder did not expand indexed output".into())
    };
    Ok((info.width,info.height,rgba))
}

fn main()->Result<(),DynError>{
    for path in std::env::args().skip(1){
        let input=std::fs::read(&path)?;
        let image=raipixel::png::decode(&input,Limits::default(),&mut Work::default())?.image;
        let [w,h]=image.size();
        let mut ours=Vec::new();
        let supported=raipixel::png::encode_indexed(&image,&mut ours,&mut Work::default())?;
        if !supported {
            assert!(ours.is_empty(),"index preflight wrote bytes");
            assert!(packed(&image)?.is_none());
            println!("{{\"supported\":false,\"width\":{},\"height\":{}}}",w,h);
            continue;
        }
        let fast=encode_ref(&image,true)?.ok_or("reference fast palette missing")?;
        let default=encode_ref(&image,false)?.ok_or("reference default palette missing")?;
        for bytes in [&ours,&fast,&default] {
            let (x,y,rgba)=reference_rgba(bytes)?;
            assert_eq!([x,y],[w,h]);
            assert_eq!(rgba.as_slice(),image.pixels().as_flattened(),
                       "independent reference palette decode differs");
            assert_eq!(raipixel::png::decode(bytes,Limits::default(),
                       &mut Work::default())?.image,image);
        }
        // Both compilers receive full-RGBA input and construct the exact palette
        // on every iteration: a whole-workflow benchmark rather than bare DEFLATE.
        let encode=|mode:usize|->Result<Vec<u8>,DynError>{
            if mode==0 {
                let mut result=Vec::new();
                assert!(raipixel::png::encode_indexed(black_box(&image),
                    &mut result,&mut Work::default())?);
                Ok(result)
            }else{
                Ok(encode_ref(black_box(&image),mode==1)?.ok_or("palette lost")?)
            }
        };
        let batch=|mode:usize,n:usize|->Result<Duration,DynError>{
            let start=Instant::now();
            for _ in 0..n{black_box(encode(mode)?);}
            Ok(start.elapsed())
        };
        let mut iterations=[1usize;3];
        for (mode,n) in iterations.iter_mut().enumerate(){
            while batch(mode,*n)?<Duration::from_millis(25){
                *n=n.checked_mul(2).ok_or("iteration overflow")?;
            }
        }
        let mut samples:[Vec<f64>;3]=std::array::from_fn(|_|Vec::new());
        for round in 0..9{
            for offset in 0..3{
                let mode=(round+offset)%3;
                samples[mode].push(batch(mode,iterations[mode])?.as_secs_f64()*1e6
                                   /iterations[mode] as f64);
            }
        }
        println!("{{\"supported\":true,\"width\":{},\"height\":{},\"png_depth\":{},\"bytes\":{:?},\"samples_us\":{:?}}}",
                w,h,ours[24], [ours.len(),fast.len(),default.len()],samples);
    }
    Ok(())
}
