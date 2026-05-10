use std::fs;

use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::{Options, Tree},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let svg_data = fs::read("arabic-script.svg")?;

    let mut opt = Options::default();
	opt.fontdb_mut().load_system_fonts();
	
    let tree = Tree::from_data(&svg_data, &opt)?;

    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height())
        .ok_or("Failed to create pixmap")?;

    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

    pixmap.save_png("output.png")?;

    println!("Rendered to output.png");
    Ok(())
}