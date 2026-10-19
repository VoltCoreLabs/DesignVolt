//! Headless performance check of the canvas: builds a page with N shapes and measures, for a few
//! zoom levels, how long it takes to build the Vello scene (CPU) and to render it (GPU).
//!
//! Run with: `cargo run --release -p dv-render --example bench -- 10000`

use std::time::Instant;

use dv_core::{Color, Command, Document, Geometry, Node, NodeKind};
use dv_geom::SpatialIndex;
use dv_render::{Viewport, paint_page};
use vello::kurbo::{Affine, Point, Rect, Vec2};
use vello::util::RenderContext;
use vello::wgpu;
use vello::{AaSupport, RenderParams, Renderer, RendererOptions, Scene};

const WORLD_W: f64 = 20_000.0;
const WORLD_H: f64 = 12_000.0;
const FRAMES: usize = 60;
const SIZE: (u32, u32) = (1920, 1080);

/// Tiny deterministic generator so every run measures the same page.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / ((1u64 << 31) as f64)
    }
}

fn make_doc(n: usize) -> Document {
    let mut doc = Document::default();
    let mut rng = Lcg(42);
    for i in 0..n {
        let (w, h) = (20.0 + rng.next() * 180.0, 20.0 + rng.next() * 180.0);
        let kind = if i % 3 == 0 {
            NodeKind::Ellipse
        } else {
            NodeKind::Rectangle
        };
        let mut node = Node::new(
            kind,
            "shape",
            Geometry {
                x: rng.next() * (WORLD_W - w),
                y: rng.next() * (WORLD_H - h),
                w,
                h,
                rotation: if i % 5 == 0 { rng.next() * 90.0 } else { 0.0 },
            },
        );
        node.fill = Some(Color::rgb(
            (rng.next() * 255.0) as u8,
            (rng.next() * 255.0) as u8,
            (rng.next() * 255.0) as u8,
        ));
        node.radius = if i % 4 == 0 { 12.0 } else { 0.0 };
        let id = doc.alloc_id();
        doc.apply(&Command::Create {
            id,
            parent: None,
            index: i,
            node,
        })
        .expect("create");
    }
    doc
}

fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = v.iter().sum::<f64>() / v.len() as f64;
    (
        avg,
        v[(v.len() * 95 / 100).min(v.len() - 1)],
        v[v.len() - 1],
    )
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);

    let t = Instant::now();
    let doc = make_doc(n);
    println!("document: {n} shapes created in {:.1} ms", ms(t));
    let t = Instant::now();
    let index = SpatialIndex::build(doc.page(), doc.revision());
    println!(
        "spatial index: built in {:.1} ms ({} entries)",
        ms(t),
        index.len()
    );

    let mut ctx = RenderContext::new();
    let dev = pollster::block_on(ctx.device(None)).expect("no compatible GPU");
    let handle = &ctx.devices[dev];
    let info = handle.adapter().get_info();
    println!(
        "gpu: {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );
    let mut renderer = Renderer::new(
        &handle.device,
        RendererOptions {
            antialiasing_support: AaSupport::area_only(),
            ..RendererOptions::default()
        },
    )
    .expect("renderer");
    let texture = handle.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d {
            width: SIZE.0,
            height: SIZE.1,
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
    let size = (SIZE.0 as f64, SIZE.1 as f64);
    let params = RenderParams {
        base_color: vello::peniko::Color::from_rgba8(14, 16, 25, 255),
        width: SIZE.0,
        height: SIZE.1,
        antialiasing_method: vello::AaConfig::Area,
    };

    let mut fit = Viewport::default();
    fit.fit(Rect::new(0.0, 0.0, WORLD_W, WORLD_H), size, 20.0);
    let center = Point::new(WORLD_W / 2.0, WORLD_H / 2.0);
    let at = |zoom: f64| Viewport {
        zoom,
        pan: Vec2::new(
            size.0 / 2.0 - center.x * zoom,
            size.1 / 2.0 - center.y * zoom,
        ),
    };
    let scenarios = [
        ("whole page", fit),
        ("zoom 25%", at(0.25)),
        ("zoom 100%", at(1.0)),
        ("zoom 400%", at(4.0)),
    ];

    println!(
        "\n{FRAMES} frames each at {}x{}; times in ms (avg / p95 / max)\n",
        SIZE.0, SIZE.1
    );
    println!(
        "{:<12} {:>7} {:>7}   {:<22} {:<22} {:<22} {:>6}",
        "scenario", "cand.", "drawn", "scene build", "gpu render", "total", "fps"
    );
    let mut scene = Scene::new();
    for (name, vp) in scenarios {
        let mut build = Vec::new();
        let mut gpu = Vec::new();
        let mut last = Default::default();
        for frame in 0..(FRAMES + 5) {
            let t = Instant::now();
            scene.reset();
            let dot = vello::peniko::Color::from_rgba8(255, 255, 255, 40);
            dv_render::paint_dots(&mut scene, &vp, size, Affine::IDENTITY, dot, dot);
            last = paint_page(&mut scene, doc.page(), &index, &vp, size, Affine::IDENTITY);
            let b = ms(t);
            let t = Instant::now();
            renderer
                .render_to_texture(&handle.device, &handle.queue, &scene, &view, &params)
                .expect("render");
            handle
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("poll");
            let g = ms(t);
            if frame >= 5 {
                build.push(b);
                gpu.push(g);
            }
        }
        let total: Vec<f64> = build.iter().zip(&gpu).map(|(a, b)| a + b).collect();
        let (b, g, t) = (stats(build), stats(gpu), stats(total));
        let f = |s: (f64, f64, f64)| format!("{:.2} / {:.2} / {:.2}", s.0, s.1, s.2);
        println!(
            "{:<12} {:>7} {:>7}   {:<22} {:<22} {:<22} {:>6.0}",
            name,
            last.candidates,
            last.drawn,
            f(b),
            f(g),
            f(t),
            1000.0 / t.0
        );
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
