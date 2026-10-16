use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use resvg::{
    self,
    tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform},
    usvg::{Options, Tree},
};

const SIZE: u32 = 144;
const EXPECTED_FLAGS: usize = 258;

fn main() -> Result<()> {
    let repo_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let source_directory = repo_root.join("assets/flags");
    let output_directory = source_directory.join("circle-png");

    let mut sources = fs::read_dir(&source_directory)
        .with_context(|| format!("reading {}", source_directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .collect::<Vec<_>>();
    sources.sort();
    if sources.len() != EXPECTED_FLAGS {
        bail!("expected {EXPECTED_FLAGS} source flag SVGs, found {}", sources.len());
    }
    fs::create_dir_all(&output_directory)
        .with_context(|| format!("creating {}", output_directory.display()))?;

    let mut mask = Mask::new(SIZE, SIZE).context("allocating circular alpha mask")?;
    let mut circle = PathBuilder::new();
    let radius = SIZE as f32 / 2.0;
    circle.push_circle(radius, radius, radius);
    let circle = circle.finish().context("building circular alpha mask")?;
    mask.fill_path(&circle, FillRule::Winding, true, Transform::default());
    let mask_pixels = mask.data();
    if mask_pixels[0] != 0
        || mask_pixels[(5 * SIZE + 18) as usize] != 0
        || mask_pixels[(SIZE / 2 * SIZE + SIZE / 2) as usize] != 255
    {
        bail!("the generated alpha mask is not a centered circle");
    }

    for source in &sources {
        let bytes = fs::read(source).with_context(|| format!("reading {}", source.display()))?;
        let tree = Tree::from_data(&bytes, &Options::default())
            .with_context(|| format!("parsing {}", source.display()))?;
        let svg_size = tree.size();
        let transform = Transform::from_scale(
            SIZE as f32 / svg_size.width(),
            SIZE as f32 / svg_size.height(),
        );
        let mut pixmap = Pixmap::new(SIZE, SIZE).context("allocating flag pixmap")?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let source_pixels = pixmap.data();
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for (index, pixel) in source_pixels.chunks_exact(4).enumerate() {
            let source_alpha = pixel[3] as u32;
            let mask_alpha = mask_pixels[index] as u32;
            let output_alpha = ((source_alpha * mask_alpha + 127) / 255) as u8;

            if output_alpha == 0 || source_alpha == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            for channel in &pixel[..3] {
                let straight = ((*channel as u32 * 255 + source_alpha / 2) / source_alpha)
                    .min(255) as u8;
                rgba.push(straight);
            }
            rgba.push(output_alpha);
        }

        let image = RgbaImage::from_raw(SIZE, SIZE, rgba).context("assembling RGBA flag image")?;
        if image.get_pixel(0, 0)[3] != 0
            || image.get_pixel(18, 5)[3] != 0
            || image.get_pixel(SIZE / 2, SIZE / 2)[3] == 0
        {
            bail!("{} did not receive the circular alpha crop", source.display());
        }
        let output = output_directory.join(source.file_stem().unwrap()).with_extension("png");
        image
            .save(&output)
            .with_context(|| format!("writing {}", output.display()))?;
    }

    println!(
        "Rendered {} circular flag PNGs ({}x{}) with resvg.",
        sources.len(),
        SIZE,
        SIZE
    );
    Ok(())
}
