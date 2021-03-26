use druid::im::{HashMap, Vector};
use druid::kurbo::{BezPath, Circle};
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, PietImage, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, Command, FontDescriptor, FontStyle, FontWeight, Handled, Lens,
    LocalizedString, Menu, MouseButton, Point, Rect, Selector, TextLayout, Vec2, WindowDesc,
    WindowId,
};

use druid::widget::Axis;

use mupdf::{Colorspace, Matrix, Pixmap};

use std::collections::BTreeMap;
use std::time::Instant;

// use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;

use crate::pdf_view::make_context_menu;
use crate::preferences::{DoubleClickAction, Preferences};
use crate::{AppState, Document};

use crate::pdf_view::PdfViewState;

use crate::pdf_view::SCROLL_DIRECTION_TOGGLE;
use crate::pdf_view::TOGGLE_CROP_MODE;

use crate::pdf_view::START_INVERSION_AREA_SELECTION;

use crate::PageNum;
use crate::UNIT_SQUARE;

pub const SHOW_GIVEN_PAGE: Selector<PageNum> = Selector::new("show-given-page");

fn lerp(a: f64, b: f64, x: f64) -> f64 {
    a + (b - a) * x
}

pub fn lerp_rect(a: &Rect, b: &Rect, x: f64) -> Rect {
    Rect {
        x0: lerp(a.x0, b.x0, x),
        x1: lerp(a.x1, b.x1, x),
        y0: lerp(a.y0, b.y0, x),
        y1: lerp(a.y1, b.y1, x),
    }
}

#[derive(PartialEq)]
enum AnimationState {
    None,
    Starting,
    Running(Instant),
}

#[derive(PartialEq)]
enum AnimationField {
    None,
    Crop(f64, f64),
    //    Scale,
    //    Direction,
}

#[derive(PartialEq, Copy, Clone, Debug)]
enum VerticalDirection {
    North,
    South,
    Neither,
}
#[derive(PartialEq, Copy, Clone, Debug)]
enum HorizontalDirection {
    West,
    East,
    Neither,
}

#[derive(PartialEq, Copy, Clone)]
enum HoverTarget {
    CropMarks(HorizontalDirection, VerticalDirection),
    ColourInversionRect(usize, Point, Point),
    None,
}

use crate::pdf_view::MouseState;

pub struct PdfTextWidget {
    last_mouse_position: Point,

    animation_state: AnimationState,
    animation_field: AnimationField,
    page_positions_before_animating: BTreeMap<PageNum, Rect>,
    page_positions_after_animating: BTreeMap<PageNum, Rect>,

    hover_target: (PageNum, HoverTarget),

    data_update: bool,

    inversion_rect_edit_pixmap: Option<Pixmap>,
}

impl PdfTextWidget {
    pub fn new() -> Self {
        PdfTextWidget {
            last_mouse_position: Point::new(-1_000_000., -1_000_000.), // remember this so we have access to it during keyboard invoked animations

            animation_state: AnimationState::None,
            animation_field: AnimationField::None,
            page_positions_before_animating: BTreeMap::<PageNum, Rect>::new(),
            page_positions_after_animating: BTreeMap::<PageNum, Rect>::new(),

            hover_target: (0, HoverTarget::None),

            data_update: true,

            inversion_rect_edit_pixmap: None,
        }
    }

    fn ensure_page_image_available(
        &mut self,
        view: &PdfViewState,
        page_number: PageNum,
        ctx: &mut PaintCtx,
    ) {
        let mut cache = view.page_image_cache.borrow_mut();
        if !cache.contains_key(&page_number) {
            cache.insert(
                page_number,
                view.get_page_image(page_number, ctx.size(), ctx),
            );
        }
    }

    fn toggle_crop_mode(&mut self, ctx: &mut EventCtx, data: &PdfViewState) {
        if self.animation_state == AnimationState::None {
            self.animation_state = AnimationState::Starting;
            let target = f64::round(1. - data.crop_weight);
            self.animation_field = AnimationField::Crop(data.crop_weight, target);

            self.page_positions_after_animating = data.layout_pages_within_visible_window(
                data.text_viewer_size,
                target,
                // make sure we have new positions for all pages currently visible
                if !self.page_positions_before_animating.is_empty() {
                    let (min, max) = min_max_keys(&self.page_positions_before_animating);
                    Some((min, max))
                } else {
                    None
                },
            );

            // if there may be more pages visible after the animation, find where they are now
            if target > data.crop_weight {
                let (min, max) = min_max_keys(&self.page_positions_after_animating);

                self.page_positions_before_animating = data.layout_pages_within_visible_window(
                    data.text_viewer_size,
                    data.crop_weight,
                    Some((min, max)),
                );
            }

            if data.crop_weight != 0. {
                // if entering crop mode, have crop handle mouse will be over already selected
                self.locate_mouse_after_layout_change(&data);
            }

            ctx.request_paint();
            ctx.request_anim_frame();
        } else {
            // if we're already animating, then animate back in the other direction?
        }
    }

    fn crop_edge_drag_motion(
        &mut self,
        pos: &Point,
        data: &mut PdfViewState,
        start_pos: Point,
        start_crop_rect: Rect,
    ) {
        data.document.doc_info_changed = true;

        let min = f64::min;
        let max = f64::max;

        if let (mouse_page, HoverTarget::CropMarks(mouse_horiz, mouse_vert)) = self.hover_target {
            if let Some(screen_rect) = self.page_positions_before_animating.get(&mouse_page) {
                let delta_x = (pos.x - start_pos.x) / screen_rect.width();
                let delta_y = (pos.y - start_pos.y) / screen_rect.height();

                let mut crop_rect = start_crop_rect;

                match mouse_vert {
                    VerticalDirection::North => {
                        crop_rect.y0 = max(0., min(crop_rect.y1 - 0.1, crop_rect.y0 + delta_y));
                    }
                    VerticalDirection::South => {
                        crop_rect.y1 = min(1., max(crop_rect.y0 + 0.1, crop_rect.y1 + delta_y));
                    }
                    _ => (),
                }
                match mouse_horiz {
                    HorizontalDirection::West => {
                        crop_rect.x0 = max(0., min(crop_rect.x1 - 0.1, crop_rect.x0 + delta_x));
                    }
                    HorizontalDirection::East => {
                        crop_rect.x1 = min(1., max(crop_rect.x0 + 0.1, crop_rect.x1 + delta_x));
                    }
                    HorizontalDirection::Neither => {
                        if mouse_vert == VerticalDirection::Neither {
                            crop_rect.y0 = max(0., min(0.9, crop_rect.y0 + delta_y));
                            crop_rect.y1 = min(1., max(0.1, crop_rect.y1 + delta_y));
                            crop_rect.x1 = min(1., max(0.1, crop_rect.x1 + delta_x));
                            crop_rect.x0 = max(0., min(0.9, crop_rect.x0 + delta_x));
                        }
                    }
                }
                data.document_info.set_page_margins(mouse_page, crop_rect);
            }
        }
    }

    fn get_page_screen_rectangle(&self, page_number: PageNum) -> Rect {
        match self.page_positions_before_animating.get(&page_number) {
            Some(r) => *r,
            None => unimplemented!(),
        }
    }

    fn page_coords_of_screen_point(
        &self,
        data: &PdfViewState,
        page_number: PageNum,
        p: Point,
    ) -> Point {
        let r = self.get_page_screen_rectangle(page_number);
        let full_crop = data
            .document_info
            .weighted_page_margins_in_normalized_coords(page_number, 1.);

        let actual_crop = lerp_rect(&UNIT_SQUARE, &full_crop, data.crop_weight);

        Point {
            x: actual_crop.x0 + actual_crop.width() * (p.x - r.x0) / r.width(),
            y: actual_crop.y0 + actual_crop.height() * (p.y - r.y0) / r.height(),
        }
    }

    fn create_color_inversion_rect(&mut self, data: &mut PdfViewState) {
        let (page_number, _) = self.hover_target;
        //if let Some(screen_rect) = self.page_positions_before_animating.get(&page_number) {
        let one_corner =
            self.page_coords_of_screen_point(data, page_number, self.last_mouse_position);

        let other_corner = Point::new(
            if one_corner.x > 0.5 { 0.4 } else { 0.6 },
            if one_corner.y > 0.5 { 0.4 } else { 0.6 },
        );

        let rs = data
            .document_info
            .color_inversion_rectangles
            .entry(page_number)
            .or_insert_with(Vector::<Rect>::new);

        rs.push_back(Rect::from_points(one_corner, other_corner));

        data.page_image_cache.borrow_mut().remove(&page_number);
        //}
    }

    fn start_color_inversion_rect_drag(
        &mut self,
        data: &mut PdfViewState,
        page_number: PageNum,
        rect_idx: usize,
        mouse_corner: Point,
        other_corner: Point,
    ) {
        let rs = data
            .document_info
            .color_inversion_rectangles
            .get_mut(&page_number)
            .expect("Color inversion rectangles not found");

        rs.remove(rect_idx);

        self.inversion_rect_edit_pixmap =
            Some(data.get_page_pixmap(page_number, Size::new(512., 512.)));

        let mouse_point_on_page =
            self.page_coords_of_screen_point(data, page_number, self.last_mouse_position);

        let offset_from_mouse = mouse_corner - mouse_point_on_page;

        data.mouse_state =
            MouseState::ColourInversionRect(page_number, offset_from_mouse, other_corner);
    }

    fn color_inversion_rect_drag_motion(
        &mut self,
        ctx: &mut EventCtx,
        data: &PdfViewState,
        page: PageNum,
        mouse_offset: Vec2,
        other_corner: Point,
    ) {
        let mouse_point_on_page =
            self.page_coords_of_screen_point(data, page, self.last_mouse_position);

        let mouse_corner = Point::new(
            f64::max(0., f64::min(1., mouse_point_on_page.x + mouse_offset.x)),
            f64::max(0., f64::min(1., mouse_point_on_page.y + mouse_offset.y)),
        );

        self.hover_target.1 = HoverTarget::ColourInversionRect(0, mouse_corner, other_corner);
        ctx.request_paint();
    }

    fn finish_color_inversion_rect_drag(
        &mut self,
        _ctx: &mut EventCtx,
        data: &mut PdfViewState,
        page: PageNum,
        mouse_offset: Vec2,
        other_corner: Point,
    ) {
        data.document.doc_info_changed = true;

        let mouse_point_on_page =
            self.page_coords_of_screen_point(data, page, self.last_mouse_position);

        let mouse_corner = Point::new(
            f64::max(0., f64::min(1., mouse_point_on_page.x + mouse_offset.x)),
            f64::max(0., f64::min(1., mouse_point_on_page.y + mouse_offset.y)),
        );

        let inv_rects = data
            .document_info
            .color_inversion_rectangles
            .get_mut(&page)
            .expect("unable to get list of colour inversion rects");
        inv_rects.push_back(Rect::from_points(mouse_corner, other_corner));

        self.inversion_rect_edit_pixmap = None;

        self.hover_target.1 =
            HoverTarget::ColourInversionRect(inv_rects.len() - 1, mouse_corner, other_corner);

        data.page_image_cache.borrow_mut().remove(&page);
    }

    fn scroll_drag(
        &mut self,
        window_id: WindowId,
        pos: Point,
        data: &mut PdfViewState,
        start_pos: Point,
        start_page_number: PageNum,
        start_page_position: f64,
    ) {
        let dx = pos.x - start_pos.x;
        let dy = pos.y - start_pos.y;
        let distance = 2. * (-dx - dy); //f64::signum(dx) * f64::signum(dy) * f64::sqrt(dx*dx + dy*dy);

        data.scroll_by(window_id, distance, start_page_number, start_page_position);
    }

    fn locate_mouse_before_layout_change(&mut self, data: &PdfViewState) -> bool {
        let old = self.hover_target;
        self.hover_target = self.locate_mouse(data, &self.page_positions_before_animating);
        if self.hover_target != old {
            return true;
        }
        false
    }

    fn locate_mouse_after_layout_change(&mut self, data: &PdfViewState) -> bool {
        let old = self.hover_target;
        self.hover_target = self.locate_mouse(data, &self.page_positions_after_animating);
        if self.hover_target != old {
            return true;
        }
        false
    }

    // returns bool to say if state changed -- pass ctx so it can call just .repaint itself?
    fn locate_mouse(
        &self,
        data: &PdfViewState,
        layout: &BTreeMap<PageNum, Rect>,
    ) -> (PageNum, HoverTarget) {
        //println!("finding mouse with crop weight {}", crop_weight);
        let mut over_page_number: Option<PageNum> = None;
        let mut near_page_number: Option<PageNum> = None;
        let mut distance = 1000000.;

        let mx = self.last_mouse_position.x;
        let my = self.last_mouse_position.y;
        for (page_number, screen_rect) in layout.iter() {
            if screen_rect.contains(self.last_mouse_position) {
                over_page_number = Some(*page_number);
                break;
            }
            let min = f64::min;
            let abs = f64::abs; // use std::f64::{min, abs}; // ¿¿¿ why this not work ???
            let dist = min(
                abs(mx - screen_rect.x0),
                min(
                    abs(mx - screen_rect.x1),
                    min(abs(my - screen_rect.y0), abs(my - screen_rect.y1)),
                ),
            );
            if dist < distance {
                distance = dist;
                near_page_number = Some(*page_number);
            }
        }
        let closest = match over_page_number {
            Some(n) => n,
            None => match near_page_number {
                Some(n) => n,
                None => {
                    println!("WRANING: no pages to hunt mouse over");
                    return self.hover_target;
                }
            },
        };

        let abs = f64::abs;
        // being over the corner of a colour inversion rect overrules crop margins
        let mpos = self.page_coords_of_screen_point(data, closest, self.last_mouse_position);

        if let Some(screen_rect) = layout.get(&closest) {
            let crop_rect = data
                .document_info
                .weighted_page_margins_in_normalized_coords(closest, 1.);

            let handel_radius_in_page_units = INVERSION_AREA_HANDLE_SIZE
                * (1. - (data.crop_weight * (1. - crop_rect.height())))
                / screen_rect.height();

            if let Some(inv_rects) = data.document_info.color_inversion_rectangles.get(&closest) {
                for (idx, r) in inv_rects.iter().enumerate() {
                    if abs(mpos.x - r.min_x()) < handel_radius_in_page_units {
                        if abs(mpos.y - r.min_y()) < handel_radius_in_page_units {
                            return (
                                closest,
                                HoverTarget::ColourInversionRect(
                                    idx,
                                    Point::new(r.min_x(), r.min_y()),
                                    Point::new(r.max_x(), r.max_y()),
                                ),
                            );
                        } else if abs(mpos.y - r.max_y()) < handel_radius_in_page_units {
                            return (
                                closest,
                                HoverTarget::ColourInversionRect(
                                    idx,
                                    Point::new(r.min_x(), r.max_y()),
                                    Point::new(r.max_x(), r.min_y()),
                                ),
                            );
                        }
                    } else if abs(mpos.x - r.max_x()) < handel_radius_in_page_units {
                        if abs(mpos.y - r.min_y()) < handel_radius_in_page_units {
                            return (
                                closest,
                                HoverTarget::ColourInversionRect(
                                    idx,
                                    Point::new(r.max_x(), r.min_y()),
                                    Point::new(r.min_x(), r.max_y()),
                                ),
                            );
                        } else if abs(mpos.y - r.max_y()) < handel_radius_in_page_units {
                            return (
                                closest,
                                HoverTarget::ColourInversionRect(
                                    idx,
                                    Point::new(r.max_x(), r.max_y()),
                                    Point::new(r.min_x(), r.min_y()),
                                ),
                            );
                        }
                    }
                }
            }

            //            let actual_crop = lerp_rect(&UNIT_SQUARE, &full_crop, crop_weight);
            let r = Rect {
                x0: screen_rect.x0 + crop_rect.x0 * screen_rect.width(),
                x1: screen_rect.x0 + crop_rect.x1 * screen_rect.width(),
                y0: screen_rect.y0 + crop_rect.y0 * screen_rect.height(),
                y1: screen_rect.y0 + crop_rect.y1 * screen_rect.height(),
            };

            let left_right = if mx < r.x0 + r.width() * CROP_HANDLE_SIZE {
                HorizontalDirection::West
            } else if mx > r.x1 - r.width() * CROP_HANDLE_SIZE {
                HorizontalDirection::East
            } else {
                HorizontalDirection::Neither
            };

            let up_down = if my < r.y0 + r.height() * CROP_HANDLE_SIZE {
                VerticalDirection::North
            } else if my > r.y1 - r.height() * CROP_HANDLE_SIZE {
                VerticalDirection::South
            } else {
                VerticalDirection::Neither
            };

            return (closest, HoverTarget::CropMarks(left_right, up_down));
        }
        self.hover_target
    }
}
use std::convert::TryInto;
const CROP_HANDLE_SIZE: f64 = 1. / 3.;

const INVERSION_AREA_HANDLE_SIZE: f64 = 20.;

fn min_max_keys<T>(map: &BTreeMap<PageNum, T>) -> (PageNum, PageNum) {
    let mut min = 100000000;
    let mut max = 0;

    for key in map.keys() {
        if *key < min {
            min = *key;
        }
        if *key > max {
            max = *key;
        }
    }

    (min, max)
}

impl PdfTextWidget {
    // layout_pages_within_index_range
}

const PAGE_MOVEMENT_ANIMATION_DURATION: f64 = 170.;

// If this widget has any child widgets it should call its event, update and layout
// (and lifecycle) methods as well to make sure it works. Some things can be filtered,
// but a general rule is to just pass it through unless you really know you don't want it.
impl Widget<PdfViewState> for PdfTextWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut PdfViewState, _env: &Env) {
        if data.text_viewer_size != ctx.size() {
            self.data_update = true;
            ctx.request_paint();
        }
        data.text_viewer_size = ctx.size();
        match event {
            Event::AnimFrame(_) => match self.animation_state {
                AnimationState::Starting => {
                    ctx.request_paint();
                    ctx.request_anim_frame();
                }
                AnimationState::Running(start_time) => {
                    let elapsed = start_time.elapsed().as_millis() as f64;

                    if elapsed < PAGE_MOVEMENT_ANIMATION_DURATION {
                        ctx.request_paint();
                        ctx.request_anim_frame();
                    } else {
                        match self.animation_field {
                            AnimationField::Crop(_, end) => {
                                data.crop_weight = end;
                                let full_crop = data
                                    .document_info
                                    .weighted_page_margins_in_normalized_coords(
                                        data.page_number,
                                        1.,
                                    );

                                let (min, max) = data.scroll_direction.major_span(full_crop);
                                data.page_position =
                                    if data.page_position < min || data.page_position > max {
                                        min + (max - min) * data.page_position
                                    } else {
                                        data.page_position
                                    };
                            }
                            // AnimationField::Scale => (),
                            // AnimationField::Direction => (),
                            AnimationField::None => (),
                        }
                        self.animation_field = AnimationField::None;
                        self.animation_state = AnimationState::None;
                        self.page_positions_before_animating =
                            self.page_positions_after_animating.clone();

                        ctx.request_paint();
                    };
                }
                _ => (),
            },

            Event::Command(cmd) => {
                if cmd.is(TOGGLE_CROP_MODE) {
                    self.toggle_crop_mode(ctx, &data);
                } else if cmd.is(SCROLL_DIRECTION_TOGGLE) {
                    data.scroll_direction = data.scroll_direction.cross();
                } else if cmd.is(START_INVERSION_AREA_SELECTION) {
                    self.create_color_inversion_rect(data);
                }
                if let Some(page_number) = cmd.get(SHOW_GIVEN_PAGE) {
                    // if data.page_number != data.overview_selected_page as i32 {
                    data.set_visible_scroll_position(ctx.window_id(), *page_number, None);
                }
            }

            Event::Wheel(_) => {
                // if e.mods.ctrl() {
                // } else {
                // }
                self.locate_mouse_before_layout_change(data);
                ctx.request_paint();
            }

            Event::MouseDown(e) => {
                if e.button.is_right() {
                    let (page_number, _) = self.hover_target;
                    let menu = make_context_menu(data, page_number);
                    ctx.show_context_menu(menu, e.pos);
                    data.ignore_next_mouse_move = true;
                }

                if e.button == MouseButton::Left {
                    if e.count == 2 {
                        match data.preferences.doubleclick_action {
                            DoubleClickAction::CropMode => {
                                self.toggle_crop_mode(ctx, &data);
                            }
                            DoubleClickAction::SwitchScrollDirection => {
                                data.scroll_direction = data.scroll_direction.cross();
                                // ctx.request_paint();
                            }
                        }
                        return;
                    }
                    let (curr_page, hover_target) = self.hover_target;
                    if let HoverTarget::ColourInversionRect(rect_idx, mouse_corner, other_corner) =
                        hover_target
                    {
                        ctx.set_active(true);
                        self.start_color_inversion_rect_drag(
                            data,
                            curr_page,
                            rect_idx,
                            mouse_corner,
                            other_corner,
                        );
                    } else if data.crop_weight == 0. {
                        let start_crop_rect = data
                            .document_info
                            .page_margins_in_normalized_coords(self.hover_target.0);
                        data.mouse_state = MouseState::CropMarginDrag {
                            start_pos: e.pos,
                            start_crop_rect,
                        };
                    } else if let Some((page, _)) = data.mouse_over_hyperlink {
                        data.history.push_back(curr_page);
                        data.set_visible_scroll_position(ctx.window_id(), page, None);
                        data.select_page(page);
                    } else {
                        data.mouse_state =
                            MouseState::ScrollPageDrag(e.pos, data.page_number, data.page_position);
                        ctx.set_active(true);
                    }
                }
            }
            Event::MouseUp(e) => {
                ctx.set_active(false);
                if e.button.is_right() {}
                if e.button.is_left() {
                    if let MouseState::ColourInversionRect(
                        page_number,
                        mouse_offset,
                        other_corner,
                    ) = data.mouse_state
                    {
                        ctx.set_active(false);
                        self.finish_color_inversion_rect_drag(
                            ctx,
                            data,
                            page_number,
                            mouse_offset,
                            other_corner,
                        );
                    }
                }
                data.mouse_state = MouseState::Undragged;
            }
            Event::MouseMove(e) => {
                if e.pos.x < 0. {
                    // on linux, after closing a menu by selecting something, a weird MouseMove is then immediately sent with a random-seeming .pos value often (but not always) with a -ve .x value
                    // this is very annoying if it lands over the Overview panel and causes the rendered page to jump
                    // ignore it
                    data.ignore_next_mouse_move = false;
                    return;
                    // todo: find out why this happens
                }
                self.last_mouse_position = e.pos;

                match data.mouse_state {
                    MouseState::Undragged => {
                        // returns true if mouse has changed quadrant (nonant?) on the page
                        if self.locate_mouse_before_layout_change(&data) {
                            ctx.request_paint();
                        }

                        if data.in_reading_mode() {
                            let (mouse_page, _) = self.hover_target;
                            let pos = self.page_coords_of_screen_point(data, mouse_page, e.pos);
                            data.check_for_hyperlinks(ctx, mouse_page, pos);
                        }
                    }
                    MouseState::CropMarginDrag {
                        start_pos,
                        start_crop_rect,
                    } => {
                        self.crop_edge_drag_motion(&e.pos, data, start_pos, start_crop_rect);
                        ctx.request_paint();
                    }
                    MouseState::ScrollPageDrag(start_pos, start_page, start_page_position) => {
                        self.scroll_drag(
                            ctx.window_id(),
                            e.pos,
                            data,
                            start_pos,
                            start_page,
                            start_page_position,
                        );
                        ctx.request_paint();
                    }
                    MouseState::ColourInversionRect(page_number, mouse_offset, other_corner) => {
                        self.color_inversion_rect_drag_motion(
                            ctx,
                            data,
                            page_number,
                            mouse_offset,
                            other_corner,
                        )
                    }
                }
            }

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
            LifeCycle::Size(_) => {
                //data.text_viewer_size = ctx.size();
            }

            LifeCycle::HotChanged(now) => {
                //println!("Heat status: {}", now);
                if !now {

                    // if data.page_number != data.overview_selected_page as i32 {
                    //     data.set_visible_scroll_position(ctx.window_id(), data.overview_selected_page as i32, 0.5);
                    // }
                    //data.hover_target = None;
                }
            }
            _ => ()
            // println!("life cycle text widget {:?} envent {:?}",
            //               ctx.window_id(),
            //               event),
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &PdfViewState,
        data: &PdfViewState,
        _env: &Env,
    ) {
        // println!("UPDATE text widget {:?}", ctx.window_id());
        // if data.scroll_direction != old_data.scroll_direction {

        // }
        if (data.preferences.brightness_inversion_amount
            - old_data.preferences.brightness_inversion_amount)
            .abs()
            > f64::EPSILON
        {
            data.page_image_cache.borrow_mut().clear();
        }
        self.data_update = true;
        ctx.request_paint();
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &PdfViewState,
        _env: &Env,
    ) -> Size {
        // println!("layout text widget {:?}", layout_ctx.window_id());

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
        // Clear the whole widget with the color of your choice
        // (ctx.size() returns the size of the layout rect we're painting in)
        // Note: ctx also has a `clear` method, but that clears the whole context,
        // and we only want to clear this widget's area.
        let size = ctx.size();
        let rect = size.to_rect();
        ctx.clip(rect);
        ctx.fill(rect, &Color::BLACK);

        // generate page layout
        // ensure pages in cache?
        if self.data_update || size != data.text_viewer_size {
            self.page_positions_before_animating =
                data.layout_pages_within_visible_window(size, data.crop_weight, None);
        }
        self.data_update = false;

        // fixme: something better her, how to iterate through collection keyed by contiguous integers
        let (min, max) = min_max_keys(&self.page_positions_before_animating);

        for key in min..=max {
            self.ensure_page_image_available(&data, key, ctx);
        }

        if self.animation_state == AnimationState::Starting {
            for key in min..=max {
                self.ensure_page_image_available(&data, key, ctx);
            }
            self.animation_state = AnimationState::Running(Instant::now());
        }

        let mut animation = 0.;
        if let AnimationState::Running(start_time) = self.animation_state {
            let elapsed = start_time.elapsed().as_millis() as f64;
            animation = f64::min(1., elapsed / PAGE_MOVEMENT_ANIMATION_DURATION);
            // https://en.wikipedia.org/wiki/Smoothstep
            animation =
                animation * animation * animation * (animation * (animation * 6. - 15.) + 10.);
        }

        // todo: order keys so main page is drawn last, so in animated transitions it's always on top
        for (page_number, screen_start_rect) in self.page_positions_before_animating.iter() {
            let mut screen_end_rect: Rect = *screen_start_rect;

            if let Some(r) = self.page_positions_after_animating.get(page_number) {
                screen_end_rect = *r;
            }

            let rect = lerp_rect(screen_start_rect, &screen_end_rect, animation);

            let mut start = data.crop_weight;
            let mut end = data.crop_weight;

            if let AnimationField::Crop(startc, endc) = self.animation_field {
                start = startc;
                end = endc;
            }
            let crop_weight = lerp(start, end, animation);

            let full_crop = data
                .document_info
                .weighted_page_margins_in_normalized_coords(*page_number, 1.);

            let (full_min, full_max) = data.scroll_direction.major_span(full_crop);

            let major_span_length = |r: &Rect| {
                let (min, max) = data.scroll_direction.major_span(*r);
                max - min
            };

            let actual_crop = lerp_rect(&UNIT_SQUARE, &full_crop, crop_weight);
            let cache = data.page_image_cache.borrow();
            let image = cache
                .get(page_number)
                .expect("Unable to retrieve page image from cache.");

            // let tmp_image = ctx
            //     .make_image(0, 0, &[], ImageFormat::Rgb)
            //     .expect("unable to make temp image");

            // Zoom + panning looks weird with naive interpolation (see https://gamedev.stackexchange.com/questions/188841/how-to-smoothly-interpolate-2d-camera-with-pan-and-zoom ) but rather than do it properly, just make sure the zoom is fixed centred on the middle of the window
            // this makes the aspect ratio slightly wrong while zooming, but is still much better than the entire page bouncing sideways

            let start_crop = lerp_rect(&UNIT_SQUARE, &full_crop, lerp(start, end, 0.));
            let (act_min, act_max) = data.scroll_direction.major_span(actual_crop);

            let p = if data.page_position < act_min || data.page_position > act_max {
                act_min + (act_max - act_min) * data.page_position
            } else {
                data.page_position
            };
            //            let p = f64::max(act_min, f64::min(act_max, data.page_position));

            // let p = f64::max(actual_crop.x0, f64::min(actual_crop.x1, data.page_position));

            let fixed = if data.scroll_direction == Axis::Horizontal {
                screen_start_rect.x0
                    + ((p - start_crop.x0) / major_span_length(&start_crop))
                        * major_span_length(&screen_start_rect)
            } else {
                screen_start_rect.y0
                    + ((p - start_crop.y0) / major_span_length(&start_crop))
                        * major_span_length(&screen_start_rect)
            };

            //tofix [minor -- only happens with very wide crop margins?] : when zooming in, if the window midline is not over the crop area of a page and so the page position needs to be adjusted, there should never be less of the page visible after the zoom than there was before it.

            //todo ? use https://docs.rs/druid/0.7.0/druid/piet/trait.RenderContext.html#tymethod.draw_image_area ?
            // would still have to mess with scale factors for drawing onto page tho

            ctx.with_save(|ctx| {
                ctx.clip(rect);

                ctx.transform(Affine::translate((
                    if *page_number == data.page_number
                        && data.scroll_direction == Axis::Horizontal
                        && data.page_position >= full_min
                        && data.page_position <= full_max
                    {
                        fixed - p * rect.width() / actual_crop.width()
                    } else {
                        rect.x0 - actual_crop.x0 * rect.width() / actual_crop.width()
                    },
                    if *page_number == data.page_number
                        && data.scroll_direction == Axis::Vertical
                        && data.page_position >= full_min
                        && data.page_position <= full_max
                    {
                        fixed - p * rect.height() / actual_crop.height()
                    } else {
                        rect.y0 - actual_crop.y0 * rect.height() / actual_crop.height()
                    },
                )));

                let image_size = Size {
                    width: rect.width() / actual_crop.width(),
                    height: rect.height() / actual_crop.height(),
                };

                let mut draw_normal = true;
                if let MouseState::ColourInversionRect(
                    inversion_page_number,
                    inversion_mouse_offset,
                    inversion_other_corner,
                ) = data.mouse_state
                {
                    if *page_number == inversion_page_number {
                        if let Some(pixmap_source) = &self.inversion_rect_edit_pixmap {
                            // .as_ref()
                            // .expect("unable to get color inversion selection image");

                            let mut pixmap =
                                pixmap_source.try_clone().expect("failed to clone pixmap");

                            let mouse_point_on_page = self.page_coords_of_screen_point(
                                data,
                                inversion_page_number,
                                self.last_mouse_position,
                            );

                            // let mouse_corner = mouse_point_on_page + inversion_mouse_offset;
                            let mouse_corner = Point::new(
                                f64::max(
                                    0.,
                                    f64::min(1., mouse_point_on_page.x + inversion_mouse_offset.x),
                                ),
                                f64::max(
                                    0.,
                                    f64::min(1., mouse_point_on_page.y + inversion_mouse_offset.y),
                                ),
                            );

                            let selection_placement =
                                Rect::from_points(mouse_corner, inversion_other_corner);

                            let w = pixmap.width() as usize;
                            let h = pixmap.height() as usize;
                            let pxls = pixmap.samples_mut();
                            let min_x = f64::round(w as f64 * selection_placement.min_x()) as usize;
                            let max_x = f64::round(w as f64 * selection_placement.max_x()) as usize;
                            let min_y = f64::round(h as f64 * selection_placement.min_y()) as usize;
                            let max_y = f64::round(h as f64 * selection_placement.max_y()) as usize;

                            for y in min_y..usize::min(max_y, h) {
                                for x in min_x..usize::min(max_x, w) {
                                    let p = (3 * (x + y * w)) as usize;
                                    pxls[p] = 255 - pxls[p];
                                    pxls[p + 1] = 255 - pxls[p + 1];
                                    pxls[p + 2] = 255 - pxls[p + 2];
                                }
                            }

                            let tmp_image = ctx
                                .make_image(
                                    pixmap.width() as usize,
                                    pixmap.height() as usize,
                                    &pixmap.samples(),
                                    ImageFormat::Rgb,
                                )
                                .expect("Unable to make druid image from mupdf pixmap");
                            ctx.draw_image(
                                &tmp_image,
                                Rect::from_origin_size((0., 0.), image_size),
                                InterpolationMode::Bilinear,
                            );

                            draw_normal = false;
                        }
                    }
                }

                if draw_normal {
                    ctx.draw_image(
                        image,
                        Rect::from_origin_size((0., 0.), image_size),
                        InterpolationMode::Bilinear,
                    );
                }

                let x0 = full_crop.x0 * image_size.width;
                let x1 = full_crop.x1 * image_size.width;
                let y0 = full_crop.y0 * image_size.height;
                let y1 = full_crop.y1 * image_size.height;

                // draw clip outline -- custom page, green
                // pink even, blue odd, mauve neither
                let alpha = ((1. - crop_weight) * 255.) as u8;

                let custom = data.document_info.has_custom_margins(*page_number);

                let color = if custom {
                    Color::rgba8(50, 200, 50, alpha)
                } else if data.document_info.are_all_pages_same() {
                    //                        Color::rgba8(200,200,200,alpha)
                    Color::rgba8(150, 120, 175, alpha)
                } else if *page_number % 2 == 0 {
                    Color::rgba8(200, 100, 150, alpha)
                } else {
                    Color::rgba8(100, 150, 200, alpha)
                };

                if custom {
                    let mut layout =
                        TextLayout::<String>::from_text("Custom margin, unique to this page");
                    layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(20.0));
                    layout.set_text_color(Color::rgb8(50, 200, 50));
                    layout.rebuild_if_needed(ctx.text(), env);
                    layout.draw(ctx, (x0, y0 - 30.));
                }

                // outline search results
                let results = data.search_results.borrow();
                if let Some(rects) = data.search_results.borrow().get(page_number) {
                    for r in rects {
                        ctx.stroke(
                            Rect {
                                x0: r.x0 * image_size.width,
                                x1: r.x1 * image_size.width,
                                y0: r.y0 * image_size.height,
                                y1: r.y1 * image_size.height,
                            },
                            &Color::rgb8(240, 150, 10),
                            2.,
                        );
                    }
                }

                ctx.stroke(Rect { x0, y0, x1, y1 }, &color, 3.0);

                if ctx.is_hot() {
                    let (mouse_page, exact_thing) = self.hover_target;
                    if mouse_page == *page_number {
                        match exact_thing {
                            HoverTarget::CropMarks(mouse_horiz, mouse_vert) => {
                                let side = |path: &mut BezPath, p, ax, ay, bx, by| {
                                    path.move_to((ax + (bx - ax) * p, ay + (by - ay) * p));
                                    path.line_to((bx + (ax - bx) * p, by + (ay - by) * p));
                                };
                                let corner = |path: &mut BezPath, p, x, y, ox, oy| {
                                    path.move_to((x + (ox - x) * p, y));
                                    path.line_to((x, y));
                                    path.line_to((x, y + (oy - y) * p));
                                };
                                let draw_crop_handles =
                                    |path: &mut BezPath, c, s| match (mouse_vert, mouse_horiz) {
                                        (North, HorizontalDirection::Neither) => {
                                            side(path, s, x0, y0, x1, y0)
                                        }
                                        (North, East) => corner(path, c, x1, y0, x0, y1),
                                        (VerticalDirection::Neither, East) => {
                                            side(path, s, x1, y0, x1, y1)
                                        }
                                        (South, East) => corner(path, c, x1, y1, x0, y0),
                                        (South, HorizontalDirection::Neither) => {
                                            side(path, s, x0, y1, x1, y1)
                                        }
                                        (South, West) => corner(path, c, x0, y1, x1, y0),
                                        (VerticalDirection::Neither, West) => {
                                            side(path, s, x0, y0, x0, y1)
                                        }
                                        (North, West) => corner(path, c, x0, y0, x1, y1),
                                        (
                                            VerticalDirection::Neither,
                                            HorizontalDirection::Neither,
                                        ) => {
                                            corner(path, c, x0, y0, x1, y1);
                                            corner(path, c, x0, y1, x1, y0);
                                            corner(path, c, x1, y0, x0, y1);
                                            corner(path, c, x1, y1, x0, y0);
                                        }
                                    };
                                use HorizontalDirection::{East, West};
                                use VerticalDirection::{North, South};

                                let mut path = BezPath::new();
                                draw_crop_handles(&mut path, 1. / 3., 1. / 3.);
                                ctx.stroke(path, &Color::rgba8(255, 50, 50, alpha), 9.);

                                let mut path2 = BezPath::new();
                                draw_crop_handles(&mut path2, 0.5, 0.);
                                ctx.stroke(path2, &Color::rgba8(255, 50, 50, alpha), 3.);
                            }
                            HoverTarget::ColourInversionRect(_, mouse_corner, other_corner) => {
                                let mp = Point::new(
                                    mouse_corner.x * image_size.width,
                                    mouse_corner.y * image_size.height,
                                );
                                let op = Point::new(
                                    other_corner.x * image_size.width,
                                    other_corner.y * image_size.height,
                                );

                                ctx.fill(
                                    Circle::new(mp, INVERSION_AREA_HANDLE_SIZE),
                                    &Color::rgba8(255, 50, 50, 255),
                                );
                                ctx.stroke(
                                    Rect::from_points(mp, op),
                                    &Color::rgba8(255, 50, 50, 150),
                                    4.,
                                );
                            }
                            _ => (),
                        }
                    }
                }
            });

            ctx.stroke(rect, &Color::GRAY, 3.0);
        }
    }
}
