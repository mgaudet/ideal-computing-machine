// Retyped from the Rust Book
extern crate num;
extern crate crossbeam;

use num::Complex;
use std::str::FromStr;

extern crate image;
use image::ColorType;
use image::png::PNGEncoder;
use std::fs::File;
use std::io::Write;

use iron::prelude::*;
use iron::status;
extern crate base64;
extern crate iron;
#[macro_use] extern crate mime;

extern crate urlencoded;
extern crate router;
use urlencoded::UrlEncodedBody;
use router::Router;

fn parse_pair<T: FromStr>(s: &str, separator: char) -> Option<(T,T)> {
    match s.find(separator) {
        None => None,
        Some(index) => { 
            match (T::from_str(&s[..index]), T::from_str(&s[index+1..])) {
                (Ok(l), Ok(r)) => Some((l,r)),
                _ => None
            }
        }
    }
}

#[test]
fn test_parse_pair() {
    assert_eq!(parse_pair::<i32>("",','), None);
    assert_eq!(parse_pair::<i32>("10,",','), None);
    assert_eq!(parse_pair::<i32>("10,12",','), Some((10,12)));
    assert_eq!(parse_pair::<i32>("8x,12",','), None);
    assert_eq!(parse_pair::<f64>("",','), None);
    assert_eq!(parse_pair::<f64>("10,",','), None);
    assert_eq!(parse_pair::<f64>("10.2,12.8",','), Some((10.2, 12.8)));
    assert_eq!(parse_pair::<f64>("8x,12",','), None);
}

fn parse_complex(s : &str) -> Option<Complex<f64>> {
    match parse_pair(s, ',') {
        Some((re, im)) => Some(Complex { re, im }), 
        None => None
    }
}

#[test]
fn test_parse_complex() {
    assert_eq!(parse_complex("1.2,-2.2"), Some(Complex { re: 1.2, im: -2.2}));
    assert_eq!(parse_complex(",3.3"), None);
}

/// Given the row and column of a pixel in the output image, return
/// the corresponing point on the complex plane. 
///
/// `bounds` is a pair giving the width and height of the image in pixels
/// `pixel` is a (column, row) pair indicating a particular pixel in that image. 
/// `upper_left` and `lower_right` parameters are points on the complex plane 
/// designating the area our image covers.
/// 
/// This allows arbitrary scaling
fn pixel_to_point(bounds: (usize, usize), 
                  pixel: (usize, usize),
                  upper_left: Complex<f64>,
                  lower_right: Complex<f64>)
    -> Complex<f64>
{
    let (width, height) = (lower_right.re - upper_left.re, 
                           upper_left.im - lower_right.im);
    Complex { 
        re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64
    }
}

#[test]
fn text_pixel_to_point() {
    assert_eq!(pixel_to_point((100,100), (25,75), 
                              Complex {re: -1.0, im: 1.0}, 
                              Complex {re: 1.0, im: -1.0}),
                Complex { re: -0.5, im: -0.5});
}

fn escape_time(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex { re: 0.0, im: 0.0};
    for i in 0..limit {
        z = z*z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i)
        }
    }
    None
}

/// Render a set into a buffer
fn render(pixels: &mut [u8],
          bounds: (usize, usize), 
          upper_left: Complex<f64>,
          lower_right: Complex<f64>)
{
    assert!(pixels.len() == bounds.0 * bounds.1);
    for row in 0..bounds.1 {
        for column in 0..bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);
            pixels[row*bounds.0 + column] = 
                match escape_time(point, 255) {
                    None => 0,
                    Some(count) => 255 - count as u8
                };
        }
    }
}

fn write_image(filename: &str, pixels: &[u8], bounds: (usize, usize))
    -> Result<(), std::io::Error>
{
    let output = File::create(filename)?;
    let encoder = PNGEncoder::new(output);
    encoder.encode(&pixels, bounds.0 as u32, bounds.1 as u32, ColorType::Gray(8))?;
    Ok(())
}

fn write_bytes(pixels: &[u8], bounds: (usize, usize))
    -> Result<Vec<u8>, std::io::Error>
{
    let mut bytes : Vec<u8> = Vec::new();
    {
        let encoder = PNGEncoder::new(&mut bytes);
        encoder.encode(&pixels, bounds.0 as u32, bounds.1 as u32, ColorType::Gray(8))?;
    }
    Ok(bytes)
}

fn base64Fractal(upper_left: Complex<f64>, lower_right: Complex<f64>, bounds: (usize,usize))
    -> String
{
   let mut pixels = vec![0; bounds.0 * bounds.1];
    let threads = 8;
    let rows_per_band = bounds.1 / threads + 1;

    {
        let bands: Vec<&mut [u8]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
        crossbeam::scope(|spawner| {
            for (i, band) in bands.into_iter().enumerate() {
                let top = rows_per_band * i;
                let height = band.len() / bounds.0;
                let band_bounds = (bounds.0, height);
                let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
                let band_lower_right = pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);

                spawner.spawn(move || {
                    render(band, band_bounds, band_upper_left, band_lower_right);
                });
            }
        });
    }


    //render(&mut pixels, bounds, upper_left, lower_right);
    //write_image(&args[1], &pixels, bounds).expect("failed to write png");
    let bytes = write_bytes(&pixels, bounds).expect("failed to write png");

    format!("data:image/png;base64,{}", base64::encode(&bytes))
}

fn get_form(_request: &mut Request) -> IronResult<Response> {
    let mut response = Response::new();

    response.set_mut(status::Ok);
    response.set_mut(mime!(Text/Html; Charset=Utf8));
    response.set_mut(r#"
        <title>Mandelbrot Viewer</title>
        <form action="/mandelbrot" method="post">
            <input type="text" name="upperleft">
            <input type="text" name="lowerright">
            <button type="submit">View Mandelbrot</button>
        </form>
        "#);

    Ok(response)
}


fn post_get_fractal(request: &mut Request) -> IronResult<Response> {
    let mut response = Response::new();

    let form_data = match request.get_ref::<UrlEncodedBody>() {
        Err(e) => {
            response.set_mut(status::BadRequest);
            response.set_mut(format!("Error parsing form data: {:?}/n", e));
            writeln!(std::io::stderr(), "Error parsing form data: {:?}/n", e).unwrap();
            return Ok(response);
        }
        Ok(map) => map
    };

    let upper_left = parse_complex(match form_data.get("upperleft") {
        None => {
            response.set_mut(status::BadRequest);
            response.set_mut(format!("form data has no upper_left parameter\n"));
            writeln!(std::io::stderr(), "form data has no upper_left parameter\n").unwrap();
            return Ok(response);
        }
        Some(upper_lefts) => &upper_lefts[0]
    });

    let lower_right = parse_complex(match form_data.get("lowerright") {
        None => {
            response.set_mut(status::BadRequest);
            response.set_mut(format!("form data has no lower_right parameter\n"));
            writeln!(std::io::stderr(), "form data has no lower_right parameter\n").unwrap();
            return Ok(response);
        }
        Some(lower_rights) => &lower_rights[0]
    });

    let upper_left_unwrapped = match upper_left {
        None => {
            response.set_mut(status::BadRequest);
            response.set_mut(format!("Could not parse upper_left parameter\n"));
            writeln!(std::io::stderr(),"Could not parse upper_left parameter\n").unwrap();
            return Ok(response);
        }
        Some(ul) => ul
    };

    let lower_right_unwrapped = match lower_right {
        None => {
            response.set_mut(status::BadRequest);
            response.set_mut(format!("Could not parse lower_right parameter\n"));
            writeln!(std::io::stderr(),"Could not parse lower_right parameter\n").unwrap();
            return Ok(response);
        }
        Some(ul) => ul
    };



    let fractal_str = base64Fractal(upper_left_unwrapped, lower_right_unwrapped, (1000,1000));

    response.set_mut(status::Ok);
    response.set_mut(mime!(Text/Html; Charset=Utf8));
    response.set_mut(format!(r#"
        <title>Mandelbrot Viewer</title>
        <img src="{}" />
        "#, fractal_str));

    Ok(response)
}

fn main() {
    let mut router = Router::new();

    router.get("/", get_form, "root");
    router.post("/mandelbrot", post_get_fractal, "mandelbrot");

    Iron::new(router).http("localhost:3000").unwrap();
}
