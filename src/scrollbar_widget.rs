use druid::kurbo::{Arc, BezPath, Circle};
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, ContextMenu, FontDescriptor, FontWeight, LocalizedString, MenuDesc,
    MenuItem, Point, Rect, Selector, TextLayout, Vec2, WindowDesc,
};

//use druid_shell::piet::Text;

use druid::piet::TextLayout as PietTextLayout; // needed for .size() on text layout

use std::collections::BTreeMap;
//mod hilbert_curve

use crate::pdf_view::PdfViewState;
use crate::preferences::ScrollbarLayout;

use crate::PageNum;

trait Scrollbar {
    fn layout(&mut self, size: Size);
    fn position(&self, idx: usize) -> Point;
    fn nearest(&self, p: Point) -> usize;
    fn connect(&self, idx: usize, path: &mut BezPath);
    fn gap_between_nodes(&self) -> f64;
    // iterator through positions?
}
use std::time::Instant;

//#[derive(Default)]
pub struct ScrollbarWidget {
    length: usize,
    scrollbar: Box<dyn Scrollbar>,
    last_size_change: Instant,
}

#[derive(Default, Debug)]
struct Fractal {
    container: Size,
    length: usize,
    columns: usize,
    gap: f64,
    square: Vec<Point>,
    per_square: usize,
    order: u32,
    origin: Point,
    scale: f64,
}

impl Fractal {
    fn with_length(length: usize) -> Self {
        Fractal {
            length,
            ..Fractal::default()
        }
    }

    fn layout_square(origin: Point, u: Vec2, v: Vec2, order: u32, acc: &mut Vec<Point>) {
        match order {
            0 => acc.push(origin + v * 0.5),
            1 => {
                acc.push(origin + u * -0.25 + v * 0.25);
                acc.push(origin + u * -0.25 + v * 0.75);
                acc.push(origin + u * 0.25 + v * 0.75);
                acc.push(origin + u * 0.25 + v * 0.25);
            }
            _ => {
                let a = 0.015;
                let b = 0.075;
                let sa = f64::sin(a);
                let ca = f64::cos(a);
                let sb = f64::sin(b);
                let cb = f64::cos(b);

                let scale = 0.5 / (ca + sb);

                let mut no;
                let mut nu;
                let mut nv;

                nu = v * cb + u * sb;
                nv = u * cb - v * sb;
                no = origin + (-u * (0. + ca + sb) + 0.5 * nu) * scale;
                Self::layout_square(no, nu * scale, nv * scale, order - 1, acc);

                nu = u * ca + v * sa;
                nv = -u * sa + v * ca;
                no = origin + (-u * (0. + ca) + v * (0. + cb) + 0.5 * nu) * scale;
                Self::layout_square(no, nu * scale, nv * scale, order - 1, acc);

                nu = u * ca - v * sa;
                nv = v * ca + u * sa;
                no = origin + (u * 0. + v * (cb + 0. + sa) + 0.5 * nu) * scale;
                Self::layout_square(no, nu * scale, nv * scale, order - 1, acc);

                nu = -v * cb + u * sb;
                nv = -u * cb - v * sb;
                no = origin + (u * (0. + ca) + v * cb + 0.5 * nu) * scale;
                Self::layout_square(no, nu * scale, nv * scale, order - 1, acc);
            }
        }
    }

    fn find_within_square(p: Point, points: &[Point]) -> usize {
        let mut closest: usize = 0;
        let mut dist = 100000000.;
        for (i, x) in points.iter().enumerate() {
            let d = p.distance(*x);
            if d < dist {
                closest = i;
                dist = d;
            }
        }
        closest
    }
    // todo - this properly

    //     if points.len() == 2 {
    //         if p.distance(points[0]) < p.distance(points[1]) {
    //             return 0
    //         } else {
    //             return 1
    //         }
    //     }

    //     let mid = points.len() / 2;
    //     let quart = points.len() / 4;

    //     if p.distance(points[quart]) < p.distance(points[points.len() - quart]) {
    //         return Fractal::find_within_square(p, &points[0 .. mid])
    //     } else {
    //         return mid + Fractal::find_within_square(p, &points[mid .. points.len()])

    //     }
    // }
}

impl Scrollbar for Fractal {
    fn layout(&mut self, size: Size) {
        if self.container != size {
            self.container = size;

            let width = f64::max(2., size.width);
            let height = f64::max(2., size.height);

            let max = f64::max(width, height);
            let min = f64::min(width, height);

            let ratio = max / min;

            let log4n = f64::log(f64::max(1., self.length as f64 / ratio), 4.);

            let old_order = self.order;
            self.order = f64::round(log4n) as u32;

            if self.order != old_order {
                self.square.clear();

                Self::layout_square(
                    Point::new(0., 0.),
                    Vec2::new(1., 0.),
                    Vec2::new(0., 1.),
                    self.order,
                    &mut self.square,
                );

                self.per_square = usize::pow(4, self.order);

                let notches = 1_u32.overflowing_shl(self.order + 1).0 as f64;
                let enlargen = notches / (notches - 2.);

                let mut extent = Rect::new(0., 0.5, 0., 0.5);
                for p in &self.square {
                    extent.x0 = extent.x0.min(p.x);
                    extent.x1 = extent.x1.max(p.x);
                    extent.y0 = extent.y0.min(p.y);
                    extent.y1 = extent.y1.max(p.y);
                }

                let w = extent.width();
                let h = extent.height();
                for p in &mut self.square {
                    #[allow(clippy::suspicious_operation_groupings)]
                    let u = (1./notches + p.x - extent.x0) / (w * enlargen);
                    let v = (1./notches + p.y - extent.y0 + (1. - (h*enlargen)) / 2.) / (w * enlargen);
                    p.x = v;
                    p.y = u;
                }
            }

            let extent = ((self.length as f64 / self.per_square.max(1) as f64) * 2.).ceil() / 2.;

            if extent > ratio {
                self.scale = max / extent;
                if height > width {
                    self.origin.x = (min - (max / extent)) / 2.;
                    self.origin.y = 0.;
                } else {
                    self.origin.y = (min - (max / extent)) / 2.;
                    self.origin.x = 0.;
                }
            } else {
                self.scale = min;
                if height > width {
                    self.origin.x = 0.;
                    self.origin.y = (max - (extent * min)) / 2.;
                } else {
                    self.origin.y = 0.;
                    self.origin.x = (max - (extent * min)) / 2.;
                }
            }

            //            println!(" {} {} {}", order, self.per_square, squares);
        }
    }

    fn position(&self, idx: usize) -> Point {
        // let u;
        // let v;

        // if self.container.height > self.container.width {
        //     v = Vec2::new(1., 0.);
        //     u = Vec2::new(0., 1.);
        // } else {
        //     u = Vec2::new(0., 1.);
        //     v = Vec2::new(1., 0.);
        // }

        let mut ox = self.origin.x;
        let mut oy = self.origin.y;
        let p = if self.square.is_empty() {
            Point::new(0.5, 0.5)
        } else {
            self.square[idx % self.per_square.max(1)]
        };

        if self.container.height > self.container.width {
            oy += (idx / self.per_square.max(1)) as f64 * self.scale;
            Point::new(ox + p.x * self.scale, oy + p.y * self.scale)
        } else {
            ox += (idx / self.per_square.max(1)) as f64 * self.scale;
            Point::new(ox + p.y * self.scale, oy + p.x * self.scale)
        }
    }

    fn nearest(&self, p: Point) -> usize {
        let square;
        let u;
        let v;
        if self.container.height > self.container.width {
            square = ((p.y - self.origin.y) / self.scale).floor();
            u = (p.y - self.origin.y - square * self.scale) / self.scale;
            v = (p.x - self.origin.x) / self.scale;
        } else {
            square = ((p.x - self.origin.x) / self.scale).floor();
            u = (p.x - self.origin.x - square * self.scale) / self.scale;
            v = (p.y - self.origin.y) / self.scale;
        }

        usize::min(
            self.length - 1,
            square as usize * self.per_square
                + Fractal::find_within_square(Point::new(v, u), &self.square[..]),
        )
    }
    fn connect(&self, idx: usize, path: &mut BezPath) {
        path.line_to(self.position(idx));
    }
    fn gap_between_nodes(&self) -> f64 {
        Fractal::position(self, 0).distance(Fractal::position(self, 1))
    }
}

#[derive(Default)]
struct Grid {
    container: Size,
    length: usize,
    columns: usize,
    gap: f64,
    origin: Point,
}

impl Grid {
    fn with_length(length: usize) -> Self {
        Grid {
            length,
            ..Grid::default()
        }
    }
}

impl Scrollbar for Grid {
    fn layout(&mut self, size: Size) {
        if self.container != size {
            self.container = size;

            let width = f64::max(2., size.width);
            let height = f64::max(2., size.height);

            if height >= width {
                let ratio = height / width;

                self.columns = f64::round(f64::sqrt(self.length as f64 / ratio)) as usize;
                self.gap = f64::min(
                    width / self.columns as f64,
                    height / f64::ceil(self.length as f64 / self.columns as f64),
                );
            } else {
                let ratio = width / height;
                self.columns = f64::ceil(f64::sqrt(self.length as f64 / ratio)) as usize;
                self.gap = f64::min(
                    height / self.columns as f64,
                    width / f64::ceil(self.length as f64 / self.columns as f64),
                );
                self.columns = f64::floor(width / self.gap) as usize;
            }

            let figure_height = self.gap * f64::ceil(self.length as f64 / self.columns as f64);
            self.origin = Point::new(
                (width - self.columns as f64 * self.gap + self.gap) / 2.,
                (height - figure_height + self.gap) / 2.,
            );
        }
    }
    fn position(&self, idx: usize) -> Point {
        self.origin
            + Vec2::new(
                (idx % self.columns) as f64 * self.gap,
                (idx / self.columns) as f64 * self.gap,
            )
    }
    fn connect(&self, idx: usize, path: &mut BezPath) {
        let p = self.position(idx);
        if idx > 0 && idx % self.columns == 0 {
            let p0 = self.position(idx - 1);
            //self.layout[i - 1];
            path.line_to((p0.x - self.gap / 2., p0.y + self.gap / 2.));
            path.line_to((p.x + self.gap / 2., p.y - self.gap / 2.));
        }
        path.line_to(p);
    }

    fn gap_between_nodes(&self) -> f64 {
        self.gap
    }

    fn nearest(&self, p: Point) -> usize {
        let x = f64::floor((p.x - self.origin.x + self.gap / 2.) / self.gap) as usize;
        let y = f64::floor((p.y - self.origin.y + self.gap / 2.) / self.gap) as usize;

        let page = y * self.columns + x;

        usize::max(0, usize::min(page, self.length - 1))
    }
}

impl ScrollbarWidget {
    pub fn with_layout_and_length(layout: ScrollbarLayout, length: usize) -> Self {
        ScrollbarWidget {
            length,
            scrollbar: if layout == ScrollbarLayout::Grid {
                Box::new(Grid::with_length(length))
            } else {
                Box::new(Fractal::with_length(length))
            },
            last_size_change: Instant::now(),
        }
    }
}

use std::convert::TryInto;

use crate::pdf_text_widget::SHOW_GIVEN_PAGE;
use crate::pdf_view::{MouseState, PageOverviewPosition};
use crate::AppState;

const COLORS: [druid::Color; 10] = [
    Color::BLACK,
    Color::rgb8(140, 70, 20),
    Color::RED,
    Color::rgb8(240, 140, 0),
    Color::YELLOW,
    Color::GREEN,
    Color::rgb8(30, 100, 250), // blue
    Color::rgb8(180, 50, 200),
    Color::GRAY,
    Color::rgb8(240, 240, 240),
];

impl Widget<PdfViewState> for ScrollbarWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut PdfViewState, _env: &Env) {
        if data.mouse_state != MouseState::Undragged {
            return;
        }
        data.scrollbar_size = ctx.size();
        match event {
            Event::MouseMove(e) => {
                if data.ignore_next_mouse_move {
                    data.ignore_next_mouse_move = false;
                    return;
                }

                // todo - fix split widget so it doesn't sent mouseover events to neighbouring containers while dragging
                if self.last_size_change.elapsed().as_millis() < 500 {
                    return;
                }

                // when dragging the area splitting border the mouse can stray over the overview panel and make pages jump distractingly
                // todo - replace this with a timer which resets every time the size changes, and ignores mouseovers for the next fraction of a second
                if e.pos.x < 5. && data.scrollbar_position == PageOverviewPosition::East
                    || e.pos.y < 5. && data.scrollbar_position == PageOverviewPosition::South
                {
                    return;
                }

                let page = self.scrollbar.nearest(e.pos);
                data.set_visible_scroll_position(ctx.window_id(), page, None);
            }
            Event::MouseDown(e) => {
                if e.button.is_right() {
                    let menu = ContextMenu::new(make_context_menu::<AppState>(data), e.pos);
                    ctx.show_context_menu(menu);
                } else {
                    data.history.push_back(data.overview_selected_page);
                    data.select_page(data.page_number);
                    ctx.request_paint();
                }
            }

            Event::Command(cmd) => match cmd {
                _ if cmd.is(SET_SCROLLBAR_LAYOUT_FRACTAL) => {
                    self.scrollbar = Box::new(Fractal::with_length(self.length));
                    data.scrollbar_layout = ScrollbarLayout::Fractal;
                }
                _ if cmd.is(SET_SCROLLBAR_LAYOUT_GRID) => {
                    self.scrollbar = Box::new(Grid::with_length(self.length));
                    data.scrollbar_layout = ScrollbarLayout::Grid;
                }
                _ => (),
            },
            _ => (), //println!("overview Event {:?} {:?}", ctx.window_id(), &event),
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &PdfViewState,
        _env: &Env,
    ) {
        match event {
            LifeCycle::HotChanged(now) => {
                if !now {
                    ctx.submit_command(
                        SHOW_GIVEN_PAGE
                            .with(data.overview_selected_page)
                            .to(druid::Target::Window(ctx.window_id())),
                    );

                    //                    data.set_visible_scroll_position(ctx.window_id(), self.overview_selected_page as i32, 0.5);
                    //data.mouse_hover_target = None;
                }
            }
            LifeCycle::Size(_) => {
                self.last_size_change = Instant::now();
            }
            _ => (), //println!("overview layout {:?} envent {:?}", ctx.window_id(), event),
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &PdfViewState,
        data: &PdfViewState,
        _env: &Env,
    ) {
        if data.page_number != old_data.page_number // start animation for this one
            || data.mouse_over_hyperlink != old_data.mouse_over_hyperlink
            || data.document_info != old_data.document_info
            || data.overview_selected_page != old_data.overview_selected_page
        {
            ctx.request_paint()
        }
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
            bc.constrain(size);

            bc.constrain(size)
        } else {
            bc.max()
        }
    }

    // The paint method gets called last, after an event flow.
    // It goes event -> update -> layout -> paint, and each method can influence the next.
    // Basically, anything that changes the appearance of a widget causes a paint.
    fn paint(&mut self, ctx: &mut PaintCtx, data: &PdfViewState, _env: &Env) {
        // Clear the whole widget with the color of your choice
        // (ctx.size() returns the size of the layout rect we're painting in)
        // Note: ctx also has a `clear` method, but that clears the whole context,
        // and we only want to clear this widget's area.
        let size = ctx.size();
        let rect = size.to_rect();
        ctx.fill(rect, &Color::BLACK);

        self.scrollbar.layout(size);

        // We can paint with a Z index, this indicates that this code will be run
        // after the rest of the painting. Painting with z-index is done in order,
        // so first everything with z-index 1 is painted and then with z-index 2 etc.
        // As you can see this(red) curve is drawn on top of the green curve
        ctx.paint_with_z_index(1, move |_ctx| {
            // let mut path = BezPath::new();
            // path.move_to((0.0, size.height));
            // path.quad_to((40.0, 50.0), (size.width, 0.0));
            // // Create a color
            // let stroke_color = Color::rgb8(128, 0, 0);
            // // Stroke the path with thickness 1.0
            // ctx.stroke(path, &stroke_color, 5.0);
        });

        let page_number: usize = data.page_number;

        // Create an arbitrary bezier path
        let mut path = BezPath::new();
        path.move_to(self.scrollbar.position(0));

        let mut path2 = BezPath::new();

        let space = self.scrollbar.gap_between_nodes();

        let mut prev:Point = self.scrollbar.position(0);
        //let mut prev_colours = false;

        for i in 0..self.length {
            self.scrollbar.connect(i, &mut path);

            let pos = self.scrollbar.position(i);
            let si = self.scrollbar.gap_between_nodes();
            let mut selpag = data.overview_selected_page;
            if let Some((link_page, _)) = data.mouse_over_hyperlink {
                if link_page != 0 {
                    selpag = link_page;
                }
            }
            let in_cache = data.page_image_cache.borrow().contains_key(&i);

            let tags = data.document_info.tag_bits(i);

            //            let arc_path = BezP

            let mut colours = Vec::<usize>::new();
            for bit in 0..=9 {
                if (tags & (1 << bit)) != 0 {
                    colours.push(bit);
                }
            }

            if data.scrollbar_layout == ScrollbarLayout::Fractal && i < self.length-1 && ! colours.is_empty() {
                path2.move_to(prev.lerp(pos, 0.5));
                path2.line_to(pos);
                path2.line_to(pos.lerp(self.scrollbar.position(i+1), 0.5))
            }
            prev = pos;

            ctx.paint_with_z_index(1, move |ctx| {
                let mut color = Color::grey(0.35);

                if in_cache {
                    color = Color::GRAY;
                }



                if colours.is_empty() {
                    ctx.fill(Circle::new(pos, si * 0.2), &color);
                } else {


//                    let path = BezPath::new();
                    let mut start_angle = 0.;

                    let sweep_angle = std::f64::consts::PI * 2. / colours.len() as f64;
                    for a in 0..colours.len() {
                        let arc = druid::kurbo::CircleSegment {
                            center: pos,
                            outer_radius: si * 0.3333,
                            inner_radius: 0.,
                            start_angle,
                            sweep_angle,
                        };
                        start_angle += sweep_angle;
                        // path.move_to(pos);
                        // path.push(arc);
                        ctx.fill(arc, &COLORS[colours[a]]);
                    }
                }

                if i == page_number {
                    ctx.stroke(Circle::new(pos, si * 0.35), &Color::grey(0.7), 3.);
                }

                //                   ctx.fill(Circle::new(pos, si * 0.15), &Color::GRAY);

                if i == selpag {
                    ctx.stroke(Circle::new(pos, si * 0.5), &Color::WHITE, 4.);
                }
            });

            if let Some(s) = data.document.check_for_bookmark(i) {
                let color = Color::WHITE;

                let label = s.clone();
                ctx.paint_with_z_index(2, move |ctx| {
                    let text = ctx.text();
                    let layout = text
                        .new_text_layout(label.to_string())
                        .font(FontFamily::SANS_SERIF, 1_f64.max(0.6 * space).round())
                        //.with_weight(FontWeight::BOLD)
                        .text_color(color)
                        .build()
                        .unwrap();

                    let sz = layout.size();
                    ctx.draw_text(&layout, (pos.x - 0.55 * sz.width, pos.y - 0.6 * sz.height));
                    ctx.draw_text(
                        &layout,
                        (pos.x - 0.55 * sz.width + 1.3, pos.y - 0.6 * sz.height),
                    );
                });
            }
        }

        let line_width = if data.scrollbar_layout == ScrollbarLayout::Grid {
            2.
        } else {
            space * 0.66
        };

        ctx.stroke(path, &Color::grey(0.3), line_width);

        if data.scrollbar_layout == ScrollbarLayout::Fractal {
            ctx.stroke(path2, &Color::grey(0.6), space*0.4);
        }

        // let mut ppath = BezPath::new();
        // ppath.quad_to((40.0, 50.0), (size.width, size.height));
        // // Create a color
        // let stroke_color = Color::rgb8(0, 128, 0);
        // // Stroke the ppath with thickness 5.0
        // ctx.stroke(ppath, &stroke_color, 5.0);

        // // Rectangles: the ppath for practical people
        // let rect = Rect::from_origin_size((10.0, 10.0), (100.0, 100.0));
        // // Note the Color:rgba8 which includes an alpha channel (7F in this case)
        // let fill_color = Color::rgba8(0x00, 0x00, 0x00, 0x7F);
        // ctx.fill(rect, &fill_color);

        // // Text is easy; in real use TextLayout should either be stored in the
        // // widget and reused, or a label child widget to manage it all.
        // // This is one way of doing it, you can also use a builder-style way.
        // let mut layout = TextLayout::<String>::from_text("hello");
        // layout.set_font(FontDescriptor::new(FontFamily::SERIF).with_size(24.0));
        // layout.set_text_color(fill_color);
        // layout.rebuild_if_needed(ctx.text(), env);

        // // Let's rotate our text slightly. First we save our current (default) context:
        // ctx.with_save(|ctx| {
        //     // Now we can rotate the context (or set a clip path, for instance):
        //     // This makes it so that anything drawn after this (in the closure) is
        //     // transformed.
        //     // The transformation is in radians, but be aware it transforms the canvas,
        //     // not just the part you are drawing. So we draw at (80.0, 40.0) on the rotated
        //     // canvas, this is NOT the same position as (80.0, 40.0) on the original canvas.
        //     ctx.transform(Affine::rotate(std::f64::consts::FRAC_PI_4));
        //     layout.draw(ctx, (80.0, 40.0));
        // });
        // // When we exit with_save, the original context's rotation is restored

        // // This is the builder-style way of drawing text.
        // let text = ctx.text();
        // let layout = text
        //     .new_text_layout("yes".to_string())
        //     .font(FontFamily::SERIF, 24.0)
        //     .text_color(Color::rgb8(128, 0, 0))
        //     .build()
        //     .unwrap();
        // ctx.draw_text(&layout, (100.0, 25.0));

        // Let's burn some CPU to make a (partially transparent) image buffer
        // let image_data = make_image_data(256, 256);
        // let image = ctx
        //     .make_image(256, 256, &image_data, ImageFormat::RgbaSeparate)
        //     .unwrap();
        // // The image is automatically scaled to fit the rect you pass to draw_image
        // ctx.draw_image(&image, size.to_rect(), InterpolationMode::Bilinear);
    }
}

pub const SET_SCROLLBAR_LAYOUT_GRID: Selector = Selector::new("set-scrollbar-layout-grid");
pub const SET_SCROLLBAR_LAYOUT_FRACTAL: Selector = Selector::new("set-scrollbar-layout-fractal");

pub fn make_context_menu<T: Data>(data: &mut PdfViewState) -> MenuDesc<T> {
    let grid_label = LocalizedString::new("Grid layout");
    let fractal_label = LocalizedString::new("Fractal layout");

    MenuDesc::empty()
        .append(
            MenuItem::new(grid_label, SET_SCROLLBAR_LAYOUT_GRID)
                .selected_if(|| data.scrollbar_layout == ScrollbarLayout::Grid),
        )
        .append(
            MenuItem::new(fractal_label, SET_SCROLLBAR_LAYOUT_FRACTAL)
                .selected_if(|| data.scrollbar_layout == ScrollbarLayout::Fractal),
        )
}
