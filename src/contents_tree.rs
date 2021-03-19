// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! An example of a custom drawing widget.
//! We draw an image, some text, a shape, and a curve.

use druid::kurbo::{Arc, BezPath, Circle};
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, FontDescriptor, FontStyle, FontWeight, LocalizedString, Point,
    Rect, TextLayout, Vec2, WindowDesc,
};

use crate::pdf_view::PdfViewState;
use crate::PageNum;

#[derive(Default)]
pub struct TableOfContentsEntry {
    title: String,
    page_number: PageNum,
    children: Vec<TableOfContentsEntry>,
    position: Complex<f64>,
}

use mupdf::outline::Outline;
use mupdf::pdf::PdfDocument;

fn build_table_of_contents(
    title: &String,
    page_number: PageNum,
    entries: &Vec<Outline>,
) -> TableOfContentsEntry {
    let children = entries
        .iter()
        .map(|e| build_table_of_contents(&e.title, e.page.unwrap_or(0) as PageNum, &e.down))
        .collect();

    TableOfContentsEntry {
        title: title.to_string(),
        page_number,
        children,
        position: ONE,
    }
}

use num_complex::Complex;

const ONE: Complex<f64> = Complex { re: 1., im: 0. };
// The Hyperbolic Browser: A Focus + Context Technique for Visualizing Large Hierarchies, John Lamping and Ramana Rao 1996
fn transform(z: Complex<f64>, (p, theta): (Complex<f64>, Complex<f64>)) -> Complex<f64> {
    (theta * z + p) / (ONE + p.conj() * theta * z)
}

fn inverse((p, theta): (Complex<f64>, Complex<f64>)) -> (Complex<f64>, Complex<f64>) {
    let theta_conj = theta.conj();
    (-theta_conj * p, theta_conj)
}

fn compose(
    p_1: Complex<f64>,
    theta_1: Complex<f64>,
    p_2: Complex<f64>,
    theta_2: Complex<f64>,
) -> (Complex<f64>, Complex<f64>) {
    let denom = theta_2 * p_1 * p_2.conj() + ONE;
    let theta = (theta_1 * theta_2 + theta_1 * p_1.conj() * p_2) / denom;
    ((theta_2 * p_1 + p_2) / denom, theta.unscale(theta.norm()))
}

fn dist(a: f64) -> f64 {
    let s = 0.06;
    let frac = (1. - s * s) * f64::sin(a) / (2. * s);
    ((frac * frac + 1.).sqrt() - frac).max(s)
}

fn layout(
    node: &mut TableOfContentsEntry,
    vertex: Complex<f64>,
    midline: Complex<f64>,
    angle: f64,
) {
    node.position = vertex;

    if node.children.len() != 0 {
        let log_grandchildren = node.children.len() as f64
            + node
                .children
                .iter()
                .fold(0., |sum, e| sum + f64::ln(1. + e.children.len() as f64));
        //        let subwedge_angle = angle / node.children.len() as f64;
        let mut ang = midline.arg() - angle;

        for child in &mut node.children {
            let subwedge_angle =
                (1. + f64::ln(1. + child.children.len() as f64)) * angle / log_grandchildren;

            ang += subwedge_angle;
            let d = dist(subwedge_angle);
            let child_midline = Complex::<f64>::from_polar(1., ang);
            let p = transform(d * child_midline, (vertex, ONE));
            let m = transform(transform(child_midline, (vertex, ONE)), (-p, ONE));
            let a = transform(
                Complex::<f64>::from_polar(1., subwedge_angle),
                (Complex::<f64> { re: -d, im: 0. }, ONE),
            )
            .arg();
            layout(child, p, m, a);
            ang += subwedge_angle;
        }
    }
}

#[derive(PartialEq)]
enum MouseState {
    Undragged,
    Dragging(Complex<f64>),
}

impl Default for MouseState {
    fn default() -> Self {
        MouseState::Undragged
    }
}

#[derive(Default)]
pub struct ContentsTree {
    container: Size,
    table_of_contents: TableOfContentsEntry,
    checked_toc: bool,
    mouse_state: MouseState,
    current_transformation: (Complex<f64>, Complex<f64>),
    former_transformation: (Complex<f64>, Complex<f64>),
}

use std::f64::consts::{FRAC_PI_2, PI, TAU};

impl ContentsTree {
    fn display(
        &self,
        ctx: &mut PaintCtx,
        env: &Env,
        origin: Point,
        scale: f64,
        entry: &TableOfContentsEntry,
        parent: Option<Complex<f64>>,
        neighbour: f64,
    ) {
        let (p, theta) = self.current_transformation;
        let end = transform(entry.position, (p, theta));

        let mut end_angle = 0.;
        let mut d = 0.;

        if let Some(start) = parent {
            let mut path = BezPath::new();
            path.move_to((origin.x + scale * start.re, origin.y + scale * start.im));
            path.line_to((origin.x + scale * end.re, origin.y + scale * end.im));

            d = start.re * end.im - end.re * start.im;
            let c = (((start * (ONE + end.norm_sqr())) - end * (ONE + start.norm_sqr()))
                * Complex::<f64>::i())
                / (2. * d);

            if d.abs() < f64::EPSILON {
                let mut path = BezPath::new();
                path.move_to((origin.x + scale * start.re, origin.y + scale * start.im));
                path.line_to((origin.x + scale * end.re, origin.y + scale * end.im));
                ctx.stroke(path, &Color::rgb8(20, 240, 240), 1.2);
            } else {
                let start_angle = (start - c).arg();
                let end_vec = end - c;
                end_angle = end_vec.arg();
                let rad = end_vec.norm() * scale;
                let mut sweep = end_angle - start_angle;

                if sweep > PI {
                    sweep = -(TAU - sweep);
                } else if sweep < -PI {
                    sweep = (TAU + sweep);
                }

                let arc = druid::kurbo::Arc {
                    center: Point {
                        x: origin.x + c.re * scale,
                        y: origin.y + c.im * scale,
                    },
                    radii: Vec2 { x: rad, y: rad },
                    //                                       outer_radius: rad+1.2,
                    //                                     inner_radius: rad,
                    start_angle,
                    sweep_angle: sweep,
                    x_rotation: 0.,
                };
                let color = if sweep < PI {
                    Color::rgb8(20, 240, 240)
                } else {
                    Color::rgb8(240, 40, 50)
                };
                ctx.stroke(arc, &color, 1.2);
            }

            //            ctx.stroke(path, &Color::rgb8(20,240,240), 1.2);
        }

        let positions: Vec<f64> = entry
            .children
            .iter()
            .map(|e| transform(e.position, self.current_transformation).im)
            .collect();

        let count = entry.children.len();
        for (i, child) in entry.children.iter().enumerate() {
            let nearest = if i == 0 && count > 1 {
                (positions[0] - positions[1]).abs()
            } else if i + 1 == count {
                if count > 1 {
                    (positions[i] - positions[i - 1]).abs()
                } else {
                    1.
                }
            } else {
                f64::min(
                    (positions[i] - positions[i.saturating_sub(1)]).abs(),
                    (positions[i] - positions[i + 1]).abs(),
                ) //;
            };
            self.display(ctx, env, origin, scale, child, Some(end), nearest);
        }
        let angle_text = self.mouse_state == MouseState::Undragged;

        let mut size = 16_f64;
        if count == 0 && !angle_text {
            size = size.min(neighbour * scale).ceil();
        }

        let mut layout = TextLayout::<String>::from_text(&entry.title); //data);
        layout.set_font(
            FontDescriptor::new(FontFamily::SANS_SERIF)
                .with_size(size)
                .with_weight(FontWeight::BOLD)
                .with_style(FontStyle::Italic),
        );
        layout.set_text_color(Color::WHITE);

        layout.rebuild_if_needed(ctx.text(), env);

        ctx.paint_with_z_index(10, move |ctx| {
            let Size { width, height } = layout.size();
            let mut x = scale * end.re;
            // if x <= 0. && count != 0{
            //     x -= width/2.;
            // }

            // if count == 0 {
            //     x += width/2.;
            // }
            //            let posn = (origin.x + x - width/2., origin.y + scale * end.im - height/2.);
            let mut posn = (origin.x + x, origin.y + scale * end.im - height / 2.);

            if d > 0. {
                end_angle -= FRAC_PI_2
            } else {
                end_angle += FRAC_PI_2
            }
            if angle_text {
                ctx.transform(Affine::translate(posn)); //Vec2{x:-posn.0,y:-posn.1}));
                ctx.transform(Affine::rotate(end_angle)); //std::f64::consts::FRAC_PI_4));
                                                          //ctx.transform(Affine::translate(posn));

                let rect = Rect::from_origin_size((0., 0.), layout.size());
                let fill_color = Color::rgba8(0x00, 0x00, 0x00, 0x7F);
                ctx.fill(rect, &fill_color);

                layout.draw(ctx, (0., 0.));
            } else {
                if end_angle > FRAC_PI_2 || end_angle < -FRAC_PI_2 {
                    posn.0 -= layout.size().width;
                }
                let rect = Rect::from_origin_size(posn, layout.size());
                // Note the Color:rgba8 which includes an alpha channel (7F in this case)
                let fill_color = Color::rgba8(0x00, 0x00, 0x00, 0x7F);
                ctx.fill(rect, &fill_color);

                layout.draw(ctx, posn)
            }
        });

        // draw label
    }
}

fn pan_hyperbolic_plane(
    start: Complex<f64>,
    end: Complex<f64>,
    (p, _): (Complex<f64>, Complex<f64>),
) -> (Complex<f64>, Complex<f64>) {
    let a = transform(start, (-p, ONE));
    let ae_conj = (a * end).conj();
    let b = Complex::<f64> {
        re: ((end - a) * (ONE + ae_conj)).re,
        im: ((end - a) * (ONE - ae_conj)).im,
    }
    .unscale(1. - (a * end).norm_sqr());
    compose(-p, ONE, b, ONE)
}

// If this widget has any child widgets it should call its event, update and layout
// (and lifecycle) methods as well to make sure it works. Some things can be filtered,
// but a general rule is to just pass it through unless you really know you don't want it.
impl Widget<PdfViewState> for ContentsTree {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut PdfViewState, _env: &Env) {
        data.contents_size = ctx.size();
        match event {
            //        LifeCycle::Size(size) => data.contents_size = size,
            Event::WindowConnected => {
                println!("build toc?");
            }
            Event::MouseDown(e) => {
                let size = ctx.size();
                let rect = size.to_rect();

                let center = rect.center();
                let r = size.width.min(size.height) / 2.;

                let mpos = Complex::<f64> {
                    re: (e.pos.x - center.x) / r,
                    im: (e.pos.y - center.y) / r,
                };
                let inverse_transform = inverse(self.current_transformation);
                let inv_mpos = transform(mpos, inverse_transform);
                if inv_mpos.norm() <= 0.999999 {
                    self.mouse_state = MouseState::Dragging(inv_mpos);
                    self.former_transformation = self.current_transformation;
                }
            }
            Event::MouseMove(e) => match self.mouse_state {
                MouseState::Dragging(start) => {
                    let size = ctx.size();
                    let rect = size.to_rect();

                    let center = rect.center();
                    let r = size.width.min(size.height) / 2.;

                    let end = Complex::<f64> {
                        re: (e.pos.x - center.x) / r,
                        im: (e.pos.y - center.y) / r,
                    };

                    if end.norm() <= 0.97 {
                        self.current_transformation =
                            pan_hyperbolic_plane(start, end, self.former_transformation);

                        ctx.request_paint();
                    }
                }
                _ => (),
            },

            Event::MouseUp(e) => match self.mouse_state {
                MouseState::Dragging(start) => {
                    self.former_transformation = self.current_transformation;
                    self.mouse_state = MouseState::Undragged;
                }
                _ => (),
            },

            _ => (),
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        _data: &PdfViewState,
        _env: &Env,
    ) {
        match event {
            //        LifeCycle::Size(size) => data.contents_size = size,
            _ => (),
        }
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx,
        _old_data: &PdfViewState,
        _data: &PdfViewState,
        _env: &Env,
    ) {
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &PdfViewState,
        _env: &Env,
    ) -> Size {
        // BoxConstraints are passed by the parent widget.
        // This method can return any Size within those constraints:
        // bc.constrain(my_size)
        //
        // To check if a dimension is infinite or not (e.g. scrolling):
        // bc.is_width_bounded() / bc.is_height_bounded()
        //
        // bx.max() returns the maximum size of the widget. Be careful
        // using this, since always make sure the widget is bounded.
        // If bx.max() is used in a scrolling widget things will probably
        // not work correctly.
        if bc.is_width_bounded() | bc.is_height_bounded() {
            let size = Size::new(100.0, 100.0);
            bc.constrain(size)
        } else {
            bc.max()
        }
    }

    // The paint method gets called last, after an event flow.
    // It goes event -> update -> layout -> paint, and each method can influence the next.
    // Basically, anything that changes the appearance of a widget causes a paint.
    fn paint(&mut self, ctx: &mut PaintCtx, data: &PdfViewState, env: &Env) {
        if !self.checked_toc {
            self.checked_toc = true;
            let toc = data.document.pdf_file.outlines();
            if let Ok(root_entries) = toc {
                self.table_of_contents =
                    build_table_of_contents(&"root".to_string(), 0, &root_entries);
            }
            layout(
                &mut self.table_of_contents,
                Complex::<f64> { re: 0., im: 0. },
                ONE,
                std::f64::consts::FRAC_PI_2,
            );
            self.current_transformation = (Complex::new(0.0, 0.), ONE);
        }

        // Clear the whole widget with the color of your choice
        // (ctx.size() returns the size of the layout rect we're painting in)
        // Note: ctx also has a `clear` method, but that clears the whole context,
        // and we only want to clear this widget's area.
        let size = ctx.size();
        let rect = size.to_rect();
        ctx.fill(rect, &Color::BLACK);

        let center = rect.center();
        let r = size.width.min(size.height) / 2.;

        ctx.stroke(Circle::new(center, r), &Color::rgb8(50, 50, 50), 10.);

        const C_ZERO: Complex<f64> = Complex { re: 0., im: 0. };

        self.display(ctx, env, center, r, &self.table_of_contents, None, 1.);

        // We can paint with a Z index, this indicates that this code will be run
        // after the rest of the painting. Painting with z-index is done in order,
        // so first everything with z-index 1 is painted and then with z-index 2 etc.
        // As you can see this(red) curve is drawn on top of the green curve
        // ctx.paint_with_z_index(1, move |ctx| {
        //     let mut path = BezPath::new();
        //     path.move_to((0.0, size.height));
        //     path.quad_to((40.0, 50.0), (size.width, 0.0));
        //     // Create a color
        //     let stroke_color = Color::rgb8(128, 0, 0);
        //     // Stroke the path with thickness 1.0
        //     ctx.stroke(path, &stroke_color, 5.0);
        // });

        // Create an arbitrary bezier path
        let mut path = BezPath::new();
        path.move_to(Point::ORIGIN);
        // path.line_to((40.0, 50.0), (size.width, size.height));
        // Create a color
        let stroke_color = Color::rgb8(0, 128, 0);
        // Stroke the path with thickness 5.0
        ctx.stroke(path, &stroke_color, 5.0);

        // Rectangles: the path for practical people
        // let rect = Rect::from_origin_size((10.0, 10.0), (100.0, 100.0));
        // // Note the Color:rgba8 which includes an alpha channel (7F in this case)
        // let fill_color = Color::rgba8(0x00, 0x00, 0x00, 0x7F);
        // ctx.fill(rect, &fill_color);

        // Text is easy; in real use TextLayout should either be stored in the
        // widget and reused, or a label child widget to manage it all.
        // This is one way of doing it, you can also use a builder-style way.
        // let mut layout = TextLayout::<String>::from_text("homp");//data);
        // layout.set_font(FontDescriptor::new(FontFamily::SERIF).with_size(24.0).with_weight(FontWeight::BOLD)
        //     .with_style(FontStyle::Italic));
        // layout.set_text_color(fill_color);

        // layout.rebuild_if_needed(ctx.text(), env);

        // Let's rotate our text slightly. First we save our current (default) context:
        ctx.with_save(|ctx| {
            // Now we can rotate the context (or set a clip path, for instance):
            // This makes it so that anything drawn after this (in the closure) is
            // transformed.
            // The transformation is in radians, but be aware it transforms the canvas,
            // not just the part you are drawing. So we draw at (80.0, 40.0) on the rotated
            // canvas, this is NOT the same position as (80.0, 40.0) on the original canvas.
            ctx.transform(Affine::rotate(std::f64::consts::FRAC_PI_4));
            // layout.draw(ctx, (80.0, 40.0));
        });
        // When we exit with_save, the original context's rotation is restored

        // This is the builder-style way of drawing text.
        // let text = ctx.text();
        // let layout = text
        //     .new_text_layout("hmph")//data.clone())
        //     .font(FontFamily::SANS_SERIF, 24.0)
        //     .text_color(Color::rgb8(128, 0, 0))
        //     .build()
        //     .unwrap();
        // ctx.draw_text(&layout, (100.0, 25.0));

        // // Let's burn some CPU to make a (partially transparent) image buffer
        // let image_data = make_image_data(256, 256);
        // let image = ctx
        //     .make_image(256, 256, &image_data, ImageFormat::RgbaSeparate)
        //     .unwrap();
        // // The image is automatically scaled to fit the rect you pass to draw_image
        // ctx.draw_image(&image, size.to_rect(), InterpolationMode::Bilinear);
    }
}

fn make_image_data(width: usize, height: usize) -> Vec<u8> {
    let mut result = vec![0; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let ix = (y * width + x) * 4;
            result[ix] = x as u8;
            result[ix + 1] = y as u8;
            result[ix + 2] = !(x as u8);
            result[ix + 3] = 127;
        }
    }
    result
}
