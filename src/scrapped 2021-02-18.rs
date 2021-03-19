
#[derive(Debug)]
enum Direction {
    Horizontal,
    Vertical,
}


impl Default for Direction {
    fn default() -> Self { Direction::Horizontal }
}

struct Document {
    pdf_document: PdfDocument,
    crop_region: druid::Rect,
}

impl Document {
    fn get_crop_rect(&self, page_number: i32) -> druid::Rect {
//        druid::Rect::new(0.25, 0.25, 0.4, 0.5)
        //druid::Rect::new(0., 0.05, 0.95, 1.)
        self.crop_region.clone()
    }

}


struct PdfDisplay;

#[derive(Default)]
struct DocumentView {
    vertical_scroll: bool,
    page_number: i32,
    page_position: f64,
    document_fingerprint: String,
    scrollbar_direction_id: u32,
    crop_mode: bool,
    rendered_pages: BTreeMap<i32, druid::piet::PietImage>,
    adjusting_inversion_page: Option<mupdf::Image>,
    anim_start: Option<std::time::Instant>,
    anim_target: f32,
    crop_amount: f64, // 0 = no cropping and full page visible, 1 = fully cropped
    is_hot: bool,

}

fn blend(a: f64, b: f64, x: f64) -> f64 { a * (1. - x) + b * x }
 
#[derive(Clone, Data)]
struct AppState {
    documents: Rc<RefCell<BTreeMap<String, Document>>>,
    queue: Rc<RefCell<Vec<String>>>,
    view_switcher_states: Rc<RefCell<BTreeMap<u32,u32>>>,
    document_views: Rc<RefCell<BTreeMap<druid::WindowId, DocumentView>>>,
    clock_ticks: usize,
    mouse_over_vertical_direction: VerticalDirection,
    mouse_over_horizontal_direction: HorizontalDirection,
    mouse_over_document: String,
    mouse_over_page: i32,
    mouse_drag_position_start: Point,
    mouse_drag_rect_start: Rect,
    mouse_drag_page_size: Size,
    mouse_drag_state: MouseState,
}

impl AppState {
    fn crop_mode_mouse_motion_not_dragging(&mut self, ctx: &mut EventCtx, event: &MouseEvent) {
        let mut document_views = self.document_views.borrow_mut();
        let view : &mut DocumentView = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");

        let mut page_number: i32 = 25;
        let page_position = 0.25;

        let docs = (self.documents).borrow_mut();
        let doc = docs.get(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

        let size = ctx.size();

        let win_parr = if view.vertical_scroll { size.height } else { size. width };
        let win_perp = if view.vertical_scroll { size. width } else { size.height };

        // let crop_rect = doc.get_crop_rect(page_number);
        // let view_rect = view.view_rect(&crop_rect);

        let page = doc.pdf_document.load_page(page_number).expect("Unable to load page");
        let page_points_size = page.bounds().expect("Unable to get page bounds");

        // let ratio = view_rect.height() / view_rect. width();
        
        // let mut view_parr = win_perp *
                // if view.vertical_scroll { view_rect.height() / view_rect.width () }
                // else                    { view_rect. width() / view_rect.height() };

        let mut view_parr = view.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);

        let mut page_start = win_parr / 2. - view_parr * view.page_offset(&view.view_rect(&doc.get_crop_rect(page_number)));

        let mpos = event.pos;

        let mut page_number = view.page_number;
        let page_count = doc.pdf_document.page_count().expect("Unable to get page count");

        let mouse_parr_coord = if view.vertical_scroll {mpos.y} else {mpos.x};
        
        if mouse_parr_coord < page_start {
            while page_number > 0 {
                page_number -= 1;
                view_parr = view.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);
                page_start -= view_parr;
                if page_start <= mouse_parr_coord { break; }
            }
        } else if mouse_parr_coord > page_start + view_parr {

            while page_number < page_count - 1 {
                page_start += view_parr;
                
                page_number += 1;
                view_parr = view.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);
                if mouse_parr_coord <= page_start + view_parr { break; }
            }
        }
        
        let x = if view.vertical_scroll { mpos.x / win_perp } 
                else                    { (mpos.x - page_start) / view_parr };
        let y = if view.vertical_scroll { (mpos.y - page_start) / view_parr }
                else                    { mpos.y / win_perp };
        //println!("over {} at normalized point {} {}", page_number, x, y);
            
        if true// view.crop_amount == 0.
         {
            let prev_mouse_document = self.mouse_over_document.clone();
            let prev_mouse_horiz = self.mouse_over_horizontal_direction;
            let prev_mouse_vert = self.mouse_over_vertical_direction;
            let prev_mouse_page = self.mouse_over_page;

            let crop_rect = doc.get_crop_rect(page_number);

            if x < crop_rect.x0 + crop_rect.width() * CROP_HANDLE_SIZE {
                self.mouse_over_horizontal_direction = HorizontalDirection::West;
            } else if x > crop_rect.x1 - crop_rect.width() * CROP_HANDLE_SIZE {
                self.mouse_over_horizontal_direction = HorizontalDirection::East;
            } else { self.mouse_over_horizontal_direction = HorizontalDirection::Neither; }

            if y < crop_rect.y0 + crop_rect.width() * CROP_HANDLE_SIZE {
                self.mouse_over_vertical_direction = VerticalDirection::North;
            } else if y > crop_rect.y1 - crop_rect.width() * CROP_HANDLE_SIZE {
                self.mouse_over_vertical_direction = VerticalDirection::South;
            } else { self.mouse_over_vertical_direction = VerticalDirection::Neither; }
            
            self.mouse_over_document = view.document_fingerprint.clone();
            self.mouse_over_page = page_number;

            if prev_mouse_document != self.mouse_over_document
            || prev_mouse_horiz != self.mouse_over_horizontal_direction
            || prev_mouse_vert != self.mouse_over_vertical_direction
            || prev_mouse_page != self.mouse_over_page 
            || view.is_hot != ctx.is_hot()
            {
                ctx.request_paint(); 
            }
            // todo: redraw other views of this document, and the document which previously had the mouse over it if that's different

            self.mouse_drag_page_size.width  = if view.vertical_scroll {  win_perp } 
                                                else                   { view_parr };
            self.mouse_drag_page_size.height = if view.vertical_scroll { view_parr } 
                                                else                   {  win_perp };
        }

    }
    fn crop_mode_mouse_drag(&mut self, ctx: &mut EventCtx, event: &MouseEvent) {
        let delta_x = (event.pos.x - self.mouse_drag_position_start.x) 
                    / self.mouse_drag_page_size.width;
        let delta_y = (event.pos.y - self.mouse_drag_position_start.y) 
                    / self.mouse_drag_page_size.height;

        let mut docs = (self.documents).borrow_mut();
        let doc = docs.get_mut(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

        let old = &self.mouse_drag_rect_start;

        match (self.mouse_over_vertical_direction) {
            VerticalDirection::North => {
                doc.crop_region.y0 = f64::max(0., f64::min(old.y1 - 0.1, old.y0 + delta_y));
            }
            VerticalDirection::South => {
                doc.crop_region.y1 = f64::min(1., f64::max(old.y0 + 0.1, old.y1 + delta_y));
            }
            _ => ()
        }
        match (self.mouse_over_horizontal_direction) {
            HorizontalDirection::East => {
                doc.crop_region.x1 = f64::min(1., f64::max(old.x0 + 0.1, old.x1 + delta_x));
            }
            HorizontalDirection::West => {
                doc.crop_region.x0 = f64::max(0., f64::min(old.x1 - 0.1, old.x0 + delta_x));
            }
            _ => {
                if self.mouse_over_vertical_direction == VerticalDirection::Neither {
                doc.crop_region.y0 = f64::max(0., f64::min(old.y1 - 0.1, old.y0 + delta_y));
                doc.crop_region.y1 = f64::min(1., f64::max(old.y0 + 0.1, old.y1 + delta_y));
                doc.crop_region.x1 = f64::min(1., f64::max(old.x0 + 0.1, old.x1 + delta_x));
                doc.crop_region.x0 = f64::max(0., f64::min(old.x1 - 0.1, old.x0 + delta_x));
                }
            }

        }
        ctx.request_paint();

        println!("pos {} {}", delta_x, delta_y);

    }

}

#[derive(Data, PartialEq, Copy, Clone)]
enum MouseState {
    Absent, Hover, Dragging
}

impl DocumentView {
    fn view_rect(&self, crop_rect: &Rect) -> Rect {
        Rect::new(crop_rect.x0 * self.crop_amount,
                  crop_rect.y0 * self.crop_amount,
                  blend(1., crop_rect.x1, self.crop_amount),
                  blend(1., crop_rect.y1, self.crop_amount))
    }

    fn page_extent_parallel_to_scroll_direction_in_screen_units(&self, win_perp: f64, page_number: i32, doc: &Document) -> f64 {

        let view_rect = self.view_rect(&doc.get_crop_rect(page_number));

        // let ratio = view_rect.height() / view_rect. width();
    
        let page = doc.pdf_document.load_page(page_number).expect("Unable to load page");
        let page_points_size = page.bounds().expect("Unable to get page bounds");

        win_perp * if self.vertical_scroll { (page_points_size.height() as f64 * view_rect.height()) 
                                           / (page_points_size.width()  as f64 * view_rect.width() ) }
                    else                   { (page_points_size.width()  as f64 * view_rect.width() ) 
                                           / (page_points_size.height() as f64 * view_rect.height()) }
    }

    fn page_offset(&self, view_rect: &Rect) -> f64 {
        if self.vertical_scroll { self.page_position - view_rect.y0 }
        else                    { self.page_position - view_rect.x0 }
    }


    fn draw_page(&mut self,
        app_state: &AppState,
        ctx: &mut PaintCtx, 
        doc: &Document, 
        page_number: i32, 
        page_position: f64, 
        screen_position: f64)
         -> (f64, f64) {
        let size = ctx.size();

        //let win_parr = if vertical_scroll { size.height } else { size. width };
        let win_perp = if self.vertical_scroll { size. width } else { size.height };

        let crop_rect = doc.get_crop_rect(page_number);
        let view_rect = self.view_rect(&crop_rect);

        self.page_position =
            if self.vertical_scroll { f64::max(view_rect.y0, f64::min(view_rect.y1, self.page_position)) }
            else                    { f64::max(view_rect.x0, f64::min(view_rect.x1, self.page_position)) };

        let page = doc.pdf_document.load_page(page_number).expect("Unable to load page");
        let page_points_size = page.bounds().expect("Unable to get page bounds");

        let scale_to_view_rect = win_perp / 
            if self.vertical_scroll { view_rect.width() * page_points_size.width() as f64 }
            else { view_rect.height() * page_points_size.height() as f64 };

        // the entire page is rendered large enough that the cropped portion fits 1:1 with pixels on the screen
        let view_rect_pixels_size = Size::new(f64::ceil(page_points_size.width()  as f64 * scale_to_view_rect),
                                              f64::ceil(page_points_size.height() as f64 * scale_to_view_rect));

        if ! self.rendered_pages.contains_key(&page_number) {
            // the entire page is rendered large enough that the cropped portion fits 1:1 with pixels on the screen
            let pixels_per_page_point = win_perp / 
                if self.vertical_scroll { crop_rect.width() * page_points_size.width() as f64 }
                else { crop_rect.height() * page_points_size.height() as f64 };

            let matrix = Matrix::new_scale(pixels_per_page_point as f32, pixels_per_page_point as f32);

            let mut pixmap = page
                .to_pixmap(&matrix, &Colorspace::device_rgb(), 0.0, true)
                .expect("Unable to render PDF page");

            // colour inversions
            let w = pixmap.width();
            let h = pixmap.height();
            let pxls = pixmap.samples_mut();
            for y in 0 .. h {
                for x in 0 .. w {
                    let p = (3 * (x + y * w)) as usize;
                    pxls[p] = 255 - pxls[p];
                    pxls[p+1] = 255 - pxls[p+1];
                    pxls[p+2] = 255 - pxls[p+2];
                }
            }

            println!("Rendered page {} at size {} x {}", page_number, w, h);

            self.rendered_pages.insert(
                page_number,
                ctx
                .make_image(pixmap.width() as usize, pixmap.height() as usize, &pixmap.samples(), ImageFormat::Rgb)
                .expect("Unable to make druid image from mupdf pixmap"));
        }

        let image = self.rendered_pages.get(&page_number).expect("Unable to find image in page cache");

        let start_position = screen_position -
            if self.vertical_scroll { (page_position - view_rect.y0) * view_rect_pixels_size.height }
            else                    { (page_position - view_rect.x0) * view_rect_pixels_size. width };

        let translate = 
            ({if self.vertical_scroll {0.} else {start_position}} - view_rect.x0 * view_rect_pixels_size.width,
             {if self.vertical_scroll {start_position} else {0.}} - view_rect.y0 * view_rect_pixels_size.height);

        ctx.with_save(|ctx| {
            ctx.transform(Affine::translate(translate));
                
            let screen_clip_area = Rect::new(
                view_rect.x0 * view_rect_pixels_size.width,
                view_rect.y0 * view_rect_pixels_size.height,
                view_rect.x1 * view_rect_pixels_size.width,
                view_rect.y1 * view_rect_pixels_size.height);
           ctx.clip(screen_clip_area);
    // let stroke_color = Color::rgb8(0,200,0);
            // ctx.stroke(screen_clip_area, &stroke_color, 3.0);

           ctx.draw_image(image, Rect::from_origin_size((0.,0.), view_rect_pixels_size), InterpolationMode::Bilinear);
        });

        let mut layer = 2;
        let mut stroke_color = Color::rgba8(200, 100, 150, ((1. - (1.*self.crop_amount)) * 250.) as u8);
            if self.document_fingerprint == app_state.mouse_over_document
            && page_number == app_state.mouse_over_page 
            && ctx.is_hot() {
                layer = 3;
                stroke_color = Color::rgba8(200, 200, 200, ((1. - (1.*self.crop_amount)) * 250.) as u8);
            };
        // Stroke the path with thickness 1.0
        let vw = view_rect_pixels_size.width;
        let vh = view_rect_pixels_size.height;
        let crop_marks = Rect::new(
                crop_rect.x0 * vw,
                crop_rect.y0 * vh,
                crop_rect.x1 * vw,
                crop_rect.y1 * vh);

        ctx.paint_with_z_index(layer, move |ctx| {
            ctx.clip(Rect::from_origin_size((0.,0.), size));

            ctx.transform(Affine::translate(translate));
            ctx.stroke(crop_marks, &stroke_color, 3.0);
        });

        let dir = (app_state.mouse_over_vertical_direction.clone(), app_state.mouse_over_horizontal_direction.clone());

        if self.document_fingerprint == app_state.mouse_over_document
        && page_number == app_state.mouse_over_page 
        && self.crop_amount == 0.
        && ctx.is_hot()
        && !(dir == (VerticalDirection::Neither, HorizontalDirection::Neither)
            && crop_rect.width() == 1. && crop_rect.height() == 1.) 
        {
            let red = Color::rgb8(250,50,50);
            let mut path = BezPath::new();
            let hw = CROP_HANDLE_SIZE * crop_rect.width();
            let hh = CROP_HANDLE_SIZE * crop_rect.height();
            let x0 = crop_rect.x0;
            let x1 = crop_rect.x1;
            let y0 = crop_rect.y0;
            let y1 = crop_rect.y1;

            ctx.paint_with_z_index(4, move |ctx| {
                ctx.clip(Rect::from_origin_size((0.,0.), size));
                ctx.transform(Affine::translate(translate));



                match dir {
                    (VerticalDirection::North, HorizontalDirection::Neither) => {
                        path.move_to((vw*(x1 - hw), vh*y0));
                        path.line_to((vw*(x0 + hw), vh*y0));
                    }
                    (VerticalDirection::North, HorizontalDirection::East) => {
                        path.move_to((vw*(x1 - hw), vh*y0));
                        path.line_to((x1 * vw, vh * y0));
                        path.line_to((vw*x1, vh*(y0 + hh)));
                    }
                    (VerticalDirection::Neither, HorizontalDirection::East) => {
                        path.move_to((vw*(x1), vh*(y0+hh)));
                        path.line_to((vw*(x1), vh*(y1-hh)));
                    }
                    (VerticalDirection::South, HorizontalDirection::East) => {
                        path.move_to((vw*(x1 - hw), vh*y1));
                        path.line_to((x1 * vw, vh * y1));
                        path.line_to((vw*x1, vh*(y1 - hh)));
                    }
                    (VerticalDirection::South, HorizontalDirection::Neither) => {
                        path.move_to((vw*(x1 - hw), vh*y1));
                        path.line_to((vw*(x0 + hw), vh*y1));
                    }
                    (VerticalDirection::South, HorizontalDirection::West) => {
                        path.move_to((vw*(x0 + hw), vh*y1));
                        path.line_to((x0 * vw, vh * y1));
                        path.line_to((vw*x0, vh*(y1 - hh)));
                    }

                    (VerticalDirection::Neither, HorizontalDirection::West) => {
                        path.move_to((vw*(x0), vh*(y0+hh)));
                        path.line_to((vw*(x0), vh*(y1-hh)));
                    }
                    (VerticalDirection::North, HorizontalDirection::West) => {
                        path.move_to((vw*(x0 + hw), vh*y0));
                        path.line_to((x0 * vw, vh * y0));
                        path.line_to((vw*x0, vh*(y0 + hh)));
                    }
                    (VerticalDirection::Neither, HorizontalDirection::Neither) => {
                        path.move_to((vw*x0, vh*y0));
                        path.line_to((vw*x1, vh*y0));
                        path.line_to((vw*x1, vh*y1));
                        path.line_to((vw*x0, vh*y1));
                        path.line_to((vw*x0, vh*y0));
                        path.line_to((vw*x1, vh*y0)); // do second corner again to avoid start and end points looking weird
                    }
                    _ => ()
                }
                ctx.stroke(path, &red, 9.);
            });
        }

        (start_position, 
         start_position + if self.vertical_scroll { view_rect.height() * view_rect_pixels_size.height }
                          else                    { view_rect. width() * view_rect_pixels_size. width } )
    }

    fn is_crop_mode(&self) -> bool { self.crop_amount == 0. }

    fn toggle_crop_mode(&mut self, ctx: &mut EventCtx, data: &AppState) {
        let docs = (data.documents).borrow_mut();
        let doc = docs.get(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

        if self.is_crop_mode() {
            self.anim_target = 1.;
 
            // let crop_rect = doc.get_crop_rect(self.page_number);
            // self.anim_start_page_position  = self.page_position;
            // self.anim_target_page_position = 
            //     if self.vertical_scroll { f64::min(crop_rect.y1, f64::max(crop_rect.y0, self.page_position)) }
            //     else                    { f64::min(crop_rect.x1, f64::max(crop_rect.x0, self.page_position)) };
        } else {
            self.anim_target = 0.;
        }

        self.anim_start = Some(std::time::Instant::now());
        ctx.request_anim_frame();
    }


    fn scroll_by(&mut self, ctx: &mut EventCtx, doc: &Document, distance: f64) {
println!("scroll {}", distance);
        // get page parallel size
        // see if we'll fit
        // if not try scrolling on next page
        let size = ctx.size();

        // let win_parr = if view.vertical_scroll { size.height } else { size. width };
        let win_perp = if self.vertical_scroll { size. width } else { size.height };

        let mut page_number = self.page_number;
        let mut view_rect = self.view_rect(&doc.get_crop_rect(page_number));
        let mut view_parr_px = self.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);
        let mut view_parr_page = if self.vertical_scroll {view_rect.height()} else {view_rect.width()};
        let mut pixels_before = ((self.page_position - if self.vertical_scroll {view_rect.y0} else {view_rect.x0})
                                 / view_parr_page)
                                * view_parr_px;
dbg!(view_parr_px - pixels_before);
dbg!(distance);
        let mut remaining = distance;

        if remaining >= 0. {
            if view_parr_px - pixels_before >= remaining {
                self.page_position += view_parr_page * remaining / view_parr_px;
            } 
            else {
                remaining -= view_parr_px - pixels_before;
                let page_count = doc.pdf_document.page_count().expect("Unable to get page count");
                while page_number < page_count - 1 {
                    page_number += 1;
                    
                    view_rect = self.view_rect(&doc.get_crop_rect(page_number));
                    view_parr_px = self.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);
                    view_parr_page = if self.vertical_scroll {view_rect.height()} else {view_rect.width()};

                    if view_parr_px > remaining {
                        self.page_number = page_number;
                        self.page_position = if self.vertical_scroll {view_rect.y0} else {view_rect.x0}
                                             + view_parr_page * remaining / view_parr_px;
                        break;
                    }
                    remaining -= view_parr_px;
                }
            }
        }
        else { // scrolling backwards
            remaining = - remaining;

            if pixels_before >= remaining {
                self.page_position -= view_parr_page * remaining / view_parr_px;
            } 
            else {
                remaining -= pixels_before;
                let page_count = doc.pdf_document.page_count().expect("Unable to get page count");
                while page_number > 0 {
                    page_number -= 1;
                    
                    view_rect = self.view_rect(&doc.get_crop_rect(page_number));
                    view_parr_px = self.page_extent_parallel_to_scroll_direction_in_screen_units(win_perp, page_number, &doc);
                    view_parr_page = if self.vertical_scroll {view_rect.height()} else {view_rect.width()};

                    if view_parr_px > remaining {
                        self.page_number = page_number;
                        self.page_position = if self.vertical_scroll {view_rect.y0} else {view_rect.x0}
                                             + view_parr_page * (view_parr_px - remaining) / view_parr_px;
                        break;
                    }
                    remaining -= view_parr_px;
                }
            }

        }


        ctx.request_paint();
    }

}

#[derive(Data, PartialEq, Copy, Clone)]
enum VerticalDirection {North, South, Neither}
#[derive(Data, PartialEq, Copy, Clone)]
enum HorizontalDirection {West, East, Neither}

const CROP_HANDLE_SIZE:f64 = 0.333333333;


// If this widget has any child widgets it should call its event, update and layout
// (and lifecycle) methods as well to make sure it works. Some things can be filtered,
// but a general rule is to just pass it through unless you really know you don't want it.
impl Widget<AppState> for PdfDisplay {

    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppState, _env: &Env) {
        match event {
            // Event::WindowConnected => {
            //     ctx.request_focus();
            // },
            Event::MouseDown(e) => {
                let mut document_views = data.document_views.borrow_mut();
                let view = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");

                if e.button == MouseButton::Left {
                    if e.count == 2 {
                        view.toggle_crop_mode(ctx, data);
                        return
                    }

                    data.mouse_drag_state = MouseState::Dragging;
                    data.mouse_drag_position_start = e.pos;
        
                    let docs = (data.documents).borrow_mut();
                    let doc = docs.get(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

                    data.mouse_drag_rect_start = doc.get_crop_rect(data.mouse_over_page);
                }
            }

            Event::MouseUp(e) => {
                if e.button == MouseButton::Left {
                    data.mouse_drag_state = MouseState::Hover;
                }
            }

            Event::MouseMove(e) => {
                if ! ctx.is_hot() {

                    data.mouse_drag_state = MouseState::Absent;
                }
                
                if data.mouse_drag_state == MouseState::Dragging {
                    data.crop_mode_mouse_drag(ctx, &e);
                } else {
                    data.crop_mode_mouse_motion_not_dragging(ctx, &e);
                }

            }
            _ => {
                println!("evnent! {:?}", event);
            }
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &AppState,
        _env: &Env,
    ) {
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &AppState, _data: &AppState, _env: &Env) {}

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &AppState,
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
    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        // below, "perp" and "parr" refer to the directions perpendicular and parallel to the direction of scrolling

        let size = ctx.size();
        let rect = size.to_rect();
        ctx.fill(rect, &Color::GRAY);
        ctx.clip(rect);


        let docs = (data.documents).borrow_mut();        
        let mut document_views = data.document_views.borrow_mut();
        let this_view : &mut DocumentView = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");
//        let this_view = document_views.entry(ctx.window_id());
               

        let this_doc = docs.get(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

        let mut cached_pages = (*this_view).rendered_pages.keys().cloned().collect::<BTreeSet<i32>>();

        let win_parr = if this_view.vertical_scroll { size.height } else { size.width  };
        let win_perp = if this_view.vertical_scroll { size.width  } else { size.height };

        let crop_rect = this_doc.get_crop_rect(this_view.page_number);
        let mut view_rect = this_view.view_rect(&crop_rect);

        // let vieww_position = if this_view.vertical_scroll 
        //                          { crop_rect.y0 + page_position * crop_rect.height() }
        //                     else { crop_rect.x0 + page_position * crop_rect.width() };

        // let view_position = if this_view.vertical_scroll { (vieww_position - view_rect.y0) / view_rect.height() }
        //                     else                         { (vieww_position - view_rect.x0) / view_rect. width() };

        let (mut screen_position_min, mut screen_position_max) = this_view.draw_page(&data,ctx,
                                                                                    &this_doc, 
                                                                                    this_view.page_number,
                                                                                    this_view.page_position,
                                                                                    win_parr / 2.);
        cached_pages.remove(&this_view.page_number);

        let mut prev_page = this_view.page_number;
        let mut next_page = this_view.page_number;

        while prev_page > 0 && screen_position_min > 0. {
            prev_page -= 1;
            view_rect = this_view.view_rect(&this_doc.get_crop_rect(prev_page));
            let (pos, _) = this_view.draw_page(&data,ctx, 
                                                &this_doc, 
                                                prev_page, 
                                                if this_view.vertical_scroll {view_rect.y1} else {view_rect.x1}, 
                                                screen_position_min);
            screen_position_min = pos;
            cached_pages.remove(&prev_page);
        }

        let page_count = this_doc.pdf_document.page_count().expect("Unable to get page count");
        let end = if this_view.vertical_scroll { size.height } else {size.width};

        while next_page + 1 < page_count && screen_position_max < end {
            next_page += 1;
            view_rect = this_view.view_rect(&this_doc.get_crop_rect(next_page));
            let (_, pos) = this_view.draw_page(&data,ctx, 
                                                &this_doc, 
                                                next_page,
                                                if this_view.vertical_scroll {view_rect.y0} else {view_rect.x0}, 
                                                screen_position_max);
            screen_position_max = pos;
            cached_pages.remove(&next_page);
        }

        for p in cached_pages {
            // println!("Deleting page image from cache: {}", p);
            // this_view.rendered_pages.remove(&p);
        }

    }
}

struct TakeFocus;

// 2021-02-07? I'm still not sure how to give input focus to widgets within ViewSwitchers when the view changes, so just handle all keyboard input here for now
impl<W: Widget<AppState>> Controller<AppState, W> for TakeFocus {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut AppState, env: &Env) {
        match event {
            Event::WindowConnected => {
                ctx.request_focus();
            },

            Event::AnimFrame(interval) => {
                let mut document_views = data.document_views.borrow_mut();
                let this_view = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");


                if let Some(instant) = this_view.anim_start {
                    let d = instant.elapsed().as_millis();
                    let anim_duration = 150;
                    if d < anim_duration {
                        let mut progress = (anim_duration - d) as f64 / anim_duration as f64;
                        
                        // https://en.wikipedia.org/wiki/Smoothstep
                       progress = progress * progress * progress * (progress * (progress * 6. - 15.) + 10.);
                        
                        if this_view.anim_target == 1.0 {
                            progress = 1.0 - progress;
                        }
                        this_view.crop_amount = progress;
                        ctx.request_paint();
                        ctx.request_anim_frame()
                    } else {
                        this_view.crop_amount = this_view.anim_target as f64;
                        this_view.anim_start = None;
                        ctx.request_paint()
                    }
                }

            }

            Event::Wheel(e) => {
                if e.mods.ctrl() {
                    println!("zoom");
                }
                else {
                    let docs = (data.documents).borrow();
                    let doc = docs.get(&("fingerprint goes here".to_string())).expect("Unable to find document from fingerprint");

                    let mut document_views = data.document_views.borrow_mut();
                    let view = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");
                    let x = e.wheel_delta.x; let y = e.wheel_delta.y;
                    let distance = f64::signum(x+y) * f64::sqrt(x*x + y*y);
                    view.scroll_by(ctx, doc, distance);
                    ctx.set_handled();
                }
            },

            Event::KeyDown(e) => {
                let mut document_views = data.document_views.borrow_mut();
                let view = document_views.get_mut(&ctx.window_id()).expect("Unable to get document view from window id");
                // n new window
                if e.key == druid::keyboard_types::Key::Character("c".to_string()) {
                    view.toggle_crop_mode(ctx, data);

                }
                else
                if e.key == druid::keyboard_types::Key::Character("=".to_string())
                || e.key == druid::keyboard_types::Key::Character("+".to_string()) {
                    println!("zoom in")
                }
                else
                if e.key == druid::keyboard_types::Key::Character("-".to_string())
                || e.key == druid::keyboard_types::Key::Character("_".to_string()) {
                    println!("zoom out")
                }
                else
                if e.key == druid::keyboard_types::Key::Character("d".to_string()) {
                    view.vertical_scroll = ! view.vertical_scroll;
                    println!("Vertical scroll: {}", view.vertical_scroll);
                    // data.clock_ticks += 1;
                ctx.request_paint();
                } 
                else
                if e.key == druid::keyboard_types::Key::Character("o".to_string())
                || e.key == druid::keyboard_types::Key::Tab
                 //&& e.mods==druid::Modifiers::CONTROL 
                 {
//                    let document_views = data.document_views.borrow();
  //                  let view = document_views.get(&ctx.window_id()).expect("Unable to get document view from window id");
                    let view_state = data.view_switcher_states.borrow_mut().entry(view.scrollbar_direction_id).and_modify(|s| *s = ((*s + 1) % 5));
                    data.clock_ticks += 1;
                    println!("{}", data.clock_ticks);
                }
                else
                if e.key == druid::keyboard_types::Key::F5
                || e.key == druid::keyboard_types::Key::Character("r".to_string()) {
                    view.rendered_pages.clear();
                    ctx.request_paint();
                }
                else
                { 
                    println!("Unhandled keypress: {:?}", e); 
                }

            },

            _ => ()
        }

        // println!("TOP LEVLE EVENT: {:?}", event);

        child.event(ctx, event, data, env)
    }
}

#[derive(Clone, Data, serde::Serialize, serde::Deserialize)]
struct CropMargins {
    distinguish_even_and_odd_pages: bool,
    even_margins: Rect,
    odd_margins: Rect,
    custom_margins: HashMap<i32, Rect>,
}

#[derive(Clone, Data, Lens)]
struct PdfDocc {
    crop_margins: CropMargins,
    // color inversion rectangles
}

#[derive(Clone, Data, Lens)]
struct PdfView {
    last_mouse_position: Point,
}

struct FingerprintIdLens(String);

// copied without understanding from https://linebender.org/druid/lens.html#getting-something-from-a-collection
impl Lens<NewAppState, Option<PdfDocc>> for FingerprintIdLens {
    fn with<R, F: FnOnce(&Option<PdfDocc>) -> R>(&self, data: &NewAppState, f: F) -> R {
        println!("with!");///////////////////////////////////////////////
        let document = data.documents.get(&self.0).cloned();
        f(&document)
    }

    fn with_mut<R, F: FnOnce(&mut Option<PdfDocc>) -> R>(&self, data: &mut NewAppState, f: F) -> R {
        println!("with_mut!");///////////////////////////////////////////////
        // get an immutable copy
        let mut document = data.documents.get(&self.0).cloned();
        let result = f(&mut document);
        // only actually mutate the collection if our result is mutated;
        let changed = match (document.as_ref(), data.documents.get(&self.0)) {
            (Some(one), Some(two)) => !one.same(two),
            (None, None) => false,
            _ => true,
        };
        if changed {
            // if !data.inner.get(&self.0).same(&document.as_ref()) {
            let documents = &mut data.documents;//Arc::make_mut(&mut data.inner);
            // if we're none, we were deleted, and remove from the map; else replace
            match document {
                Some(document) => documents.insert(self.0.clone(), document),
                None => documents.remove(&self.0),
            };
        }
        result
    }
}

#[derive(Clone, Data, Lens)]
struct WindowState {
    page_number: i32,
    page_position: f64,
    sidebar_position: u32,
    document: Option<PdfDocc>,
}

impl WindowState {
    pub fn new(global_state: Option<PdfDocc>) -> Self {
        WindowState {
            page_number: 0,
            page_position: 0.0,
            sidebar_position: 0,
            document: global_state
        }
    }
}

#[derive(Clone, Data, Lens)]
struct NewAppState {
    fingerprint: String,
    documents: HashMap<String, PdfDocc>,
}

impl NewAppState {
    pub fn new() -> Self {
        NewAppState {
            fingerprint: "fffingerpriiints".to_string(),
            documents: HashMap::<String, PdfDocc>::new(),
        }
    }
}

fn new_ui() -> impl Widget<NewAppState> {
    let scope = Scope::from_lens(
        WindowState::new,
        WindowState::document,
        // Painter::new(|ctx, woo: &WindowState, env| {
        //         let bounds = ctx.size().to_rect();
        //         println!("{}", woo.page_number);
        //         ctx.fill(bounds, &Color::BLUE);
        //     })
        ViewSwitcher::new(
            |data: &WindowState, _env| {
                data.sidebar_position
            },
            |selector, data: &WindowState, _env| match selector {
                0 => 
                Box::new(
                Painter::new(|ctx, woo: &WindowState, env| {
                        let bounds = ctx.size().to_rect();
                        println!("{}", woo.page_number);
                        ctx.fill(bounds, &Color::GREEN);
                    })),

                _ => 

                Box::new(Painter::new(|ctx, woo: &WindowState, env| {
                        let bounds = ctx.size().to_rect();
                        println!("{}", woo.page_number);
                        ctx.fill(bounds, &Color::RED);
                    }))

            }
        )
    );

    scope.lens(FingerprintIdLens("hello".to_string()))

}

fn make_ui(scrollbar_direction_id: u32) -> impl Widget<AppState> {
    Padding::new(1.,
        ViewSwitcher::new(
            move |data: &AppState, _env| {
                println!("checking switcher id {}", data.view_switcher_states.borrow().get(&scrollbar_direction_id).unwrap());
                match data.view_switcher_states.borrow().get(&scrollbar_direction_id) {
                    Some(val) => *val,
                    None => 0
                }
            },
            |selector, _data, _env| match selector {
                1 => Box::new(
                        Container::new(
                            Split::columns(
                                PdfDisplay,
                                HilbertCurve::new())
                            .split_point(0.8)
                            .draggable(true)
                            .solid_bar(true))),

                2 => Box::new(
                        Container::new(
                            Split::rows(
                                PdfDisplay,
                                HilbertCurve::new())
                            .split_point(0.8)
                            .draggable(true)
                            .solid_bar(true))),

                3 => Box::new(
                        Container::new(
                            Split::columns(
                                HilbertCurve::new(),
                                PdfDisplay)
                            .split_point(0.2)
                            .draggable(true)
                            .solid_bar(true))),


                4 => Box::new(PdfDisplay),
                
                _ => Box::new(
                        Container::new(
                            Split::rows(
                                HilbertCurve::new(),
                                PdfDisplay)
                            .split_point(0.2)
                            .draggable(true)
                            .solid_bar(true))),
            }
        )
    ).controller(TakeFocus {})
}

pub fn main() {

   let path = std::path::Path::new(r"mupdf_explored.pdf").to_slash().unwrap();
   //let path = std::path::Path::new(r"/home/oliver/books/art/illustration/Andrew Loomis - Creative.Illustration 290.pdf").to_slash().unwrap();
    // let path = std::path::Path::new(r"/home/oliver/books/graphics/unreal/Mastering Game Development with Unreal Engine 4 by Matt Edmonds.pdf").to_slash().unwrap();
    // let path = std::path::Path::new(r"/home/oliver/books/rust/Rust_in_Action_v15.pdf").to_slash().unwrap();

    let document = PdfDocument::open(&path).unwrap();


    // let last = mupdf::Rect{x0: 0., y0: 0.,x1:0.,y1:0.};
    // for pi in 0..document.page_count().expect("Could not get page count") {
    //     let p = document.load_page(pi).expect("Unable to render page");
    //     let rect = p.bounds().expect("Unable to get page bounds");
    //     println!("page {:?}", rect);
    // }

    let my_document = Document{pdf_document: document, crop_region: druid::Rect::new(0.,0.,1.,1.)};

    //let mut new_widget = PdfDisplay::default();
    //new_widget.document_fingerprint = "fingerprint goes here".to_string();
    //dbg!(win.crop_region);
    let fingerprint = "fingerprint goes here".to_string();

    let state = AppState {
        documents: Rc::new(RefCell::new(BTreeMap::<String, Document>::new())),
        queue: Rc::new(RefCell::new(Vec::<String>::new())),
        view_switcher_states: Rc::new(RefCell::new(BTreeMap::<u32,u32>::new())),
        document_views: Rc::new(RefCell::new(BTreeMap::<druid::WindowId, DocumentView>::new())),
        clock_ticks: 0,
        mouse_over_document: "".to_string(),
        mouse_over_page: -1,
        mouse_over_vertical_direction: VerticalDirection::Neither,
        mouse_over_horizontal_direction: HorizontalDirection::Neither,
        mouse_drag_position_start: Point::default(),
        mouse_drag_rect_start: Rect::default(),
        mouse_drag_page_size: Size::default(),
        mouse_drag_state: MouseState::Absent,

    };
    state.documents.borrow_mut().insert("fingerprint goes here".to_string(), my_document);
    state.queue.borrow_mut().push("fingerprint goes here".to_string());

    let scrollbar_direction_id = state.view_switcher_states.borrow().len() as u32;
    let next_scrollbar_direction_id = scrollbar_direction_id;

//    let window = WindowDesc::new(move || {make_ui(next_scrollbar_direction_id)}).title(LocalizedString::new("Fancy Colors"));
    
    let window = WindowDesc::new(new_ui).title(LocalizedString::new("Fancy Colors"));
    
    let mut new_view = DocumentView::default();
    new_view.scrollbar_direction_id = scrollbar_direction_id;
    new_view.document_fingerprint = "fingerprint goes here".to_string();
    new_view.crop_amount = 0.;
    new_view.anim_target = 0.;
    new_view.page_number = 25;
    new_view.page_position = 0.25;


    state.document_views.borrow_mut().insert(window.id, new_view);
    state.view_switcher_states.borrow_mut().insert(scrollbar_direction_id, 0);
    // println!("stadt {}", state.);

    AppLauncher::with_window(window)
        .use_simple_logger()
        .launch(NewAppState::new())
        .expect("launch failed");
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
