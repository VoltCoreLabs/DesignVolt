//! Headless check that the dot grid really reaches the screen, near the origin and millions of
//! units away from it: renders the grid on the GPU, reads the pixels back and counts the dots.
//!
//! Run with: `cargo run --release -p dv-render --example gridcheck`

use dv_render::{Viewport, paint_dots};
use vello::kurbo::{Affine, Vec2};
use vello::peniko::Color;
use vello::util::RenderContext;
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

const W: u32 = 640;
const H: u32 = 384;

fn render(ctx: &RenderContext, dev: usize, renderer: &mut Renderer, vp: &Viewport) -> Vec<u8> {
    let handle = &ctx.devices[dev];
    let device = &handle.device;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("gridcheck target"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut scene = Scene::new();
    let white = Color::from_rgba8(255, 255, 255, 255);
    paint_dots(
        &mut scene,
        vp,
        (W as f64, H as f64),
        Affine::IDENTITY,
        white,
        white,
    );
    let params = RenderParams {
        base_color: Color::from_rgba8(0, 0, 0, 255),
        width: W,
        height: H,
        antialiasing_method: AaConfig::Area,
    };
    renderer
        .render_to_texture(device, &handle.queue, &scene, &view, &params)
        .expect("render");

    let bytes_per_row = W * 4; // 2560, a multiple of 256
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gridcheck readback"),
        size: (bytes_per_row * H) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    handle.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = slice.get_mapped_range().expect("range").to_vec();
    buffer.unmap();
    data
}

/// Number of connected bright blobs (dots) and the share of lit pixels.
fn dots(pixels: &[u8]) -> (usize, f64) {
    let lit: Vec<bool> = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[0] > 60)
        .collect();
    let mut seen = vec![false; lit.len()];
    let mut blobs = 0;
    for start in 0..lit.len() {
        if !lit[start] || seen[start] {
            continue;
        }
        blobs += 1;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % W as usize, i / W as usize);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < W as usize && ny < H as usize {
                    let j = ny * W as usize + nx;
                    if lit[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
    }
    let share = lit.iter().filter(|b| **b).count() as f64 / lit.len() as f64;
    (blobs, share)
}

fn main() {
    let mut ctx = RenderContext::new();
    let dev = pollster::block_on(ctx.device(None)).expect("no compatible GPU");
    let mut renderer = Renderer::new(
        &ctx.devices[dev].device,
        RendererOptions {
            antialiasing_support: AaSupport::area_only(),
            ..RendererOptions::default()
        },
    )
    .expect("renderer");

    let cases = [
        (
            "near the origin",
            Viewport {
                zoom: 1.0,
                pan: Vec2::new(100.0, 60.0),
            },
        ),
        (
            "5 million units to the left",
            Viewport {
                zoom: 1.0,
                pan: Vec2::new(5.0e6, 0.0),
            },
        ),
        (
            "5 million units down, zoom 4",
            Viewport {
                zoom: 4.0,
                pan: Vec2::new(0.0, -2.0e7),
            },
        ),
        (
            "zoomed out to 5 %",
            Viewport {
                zoom: 0.05,
                pan: Vec2::new(-1.0e5, 3.0e4),
            },
        ),
    ];
    println!("{:<32} {:>8} {:>10}", "view", "dots", "lit pixels");
    let mut failed = false;
    for (name, vp) in cases {
        let (blobs, share) = dots(&render(&ctx, dev, &mut renderer, &vp));
        println!("{name:<32} {blobs:>8} {:>9.2}%", share * 100.0);
        // A 640x384 canvas shows at least ~ (640/28) x (384/28) = 22 x 13 dots.
        if blobs < 150 {
            failed = true;
        }
    }
    if failed {
        eprintln!("FAILED: the grid is missing in some view");
        std::process::exit(1);
    }
    println!("ok: the dot grid is rendered everywhere");
}
