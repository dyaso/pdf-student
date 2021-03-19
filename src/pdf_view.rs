use druid::im::{HashMap, Vector};
use druid::kurbo::BezPath;
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, PietImage, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, Command, ContextMenu, FileDialogOptions, FileSpec, FontDescriptor,
    FontStyle, FontWeight, Handled, Lens, LocalizedString, MenuDesc, MenuItem, MouseButton,
    MouseEvent, Point, Rect, Selector, SysMods, Target, TextLayout, Vec2, WindowDesc, WindowId,
};

use druid::widget::{
    Align, Axis, Container, Controller, Flex, Label, LineBreaking, Padding, Painter, RadioGroup,
    Scope, ScopeTransfer, Slider, Split, TextBox, ViewSwitcher, WidgetExt,
};

use druid::commands::{COPY, CUT, PASTE, SHOW_PREFERENCES, UNDO};
use druid::keyboard_types::Key;

use mupdf::{Colorspace, Matrix, Pixmap};

use std::collections::BTreeMap;
use std::time::Instant;

// use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;

use crate::preferences::{DoubleClickAction, Preferences, ScrollbarLayout};
use crate::{Document, DocumentInfo};

use crate::UNIT_SQUARE;

use crate::PageNum;

// should these go where the behaviour they invoke is implemented?
pub const TOGGLE_CROP_MODE: Selector = Selector::new("toggle-crop-mode");
pub const SCROLL_DIRECTION_TOGGLE: Selector = Selector::new("scroll-direction-toggle");
pub const REFRESH_PAGE_IMAGES: Selector = Selector::new("refresh-page-images");
pub const CUSTOMIZE_PAGE_CROP: Selector<PageNum> = Selector::new("customize-page-crop");
pub const START_INVERSION_AREA_SELECTION: Selector = Selector::new("start-inversion-selection");
pub const NEW_VIEW: Selector = Selector::new("new-view");
pub const NEW_VIEW_WITH_PARENT: Selector<crate::pdf_view::PdfViewState> =
    Selector::new("new-view-with-parent");
pub const TOGGLE_EVEN_ODD_PAGE_DISTINCTION: Selector<PageNum> =
    Selector::new("toggle-even-odd-page-distinction");
pub const REMOVE_COLOR_INVERSION_RECTANGLE: Selector<PageNum> =
    Selector::new("remove-color-inversion-rectangle");
pub const SHOW_BOOK_INFO: Selector<usize> = Selector::new("show-book-info");
pub const SAVE_DOCUMENT_INFO: Selector<Fingerprint> = Selector::new("save-document-info");
pub const REPOSITION_OVERVIEW: Selector = Selector::new("reposition-overview");

use crate::pdf_text_widget::lerp_rect;
use crate::AppState;
use crate::Fingerprint;
use crate::PdfTextWidget;
use crate::ScrollbarWidget;

#[derive(Debug, Clone, Data, PartialEq)]
pub enum MouseState {
    Undragged,
    CropMarginDrag {
        start_pos: Point,
        start_crop_rect: Rect,
    },
    ScrollPageDrag(Point, PageNum, f64),
    ColourInversionRect(PageNum, Vec2, Point),
}

#[derive(Copy, Clone, Debug, Data, PartialEq)]
pub enum PageOverviewPosition {
    Nowhere,
    North,
    South,
    East,
    West,
}

impl PageOverviewPosition {
    fn next(&mut self) -> Self {
        use PageOverviewPosition::*;
        match *self {
            Nowhere => East,
            West => North,
            North => East,
            East => South,
            South => Nowhere,
        }
    }
}

pub enum SearchState {
    NotSearching,
    Searching (PageNum,PageNum)
}

#[derive(Clone, Data, Lens)]
pub struct PdfViewState {
    pub docu_idx: usize,
    pub document: Document,
    pub document_info: DocumentInfo,

    pub mouse_state: MouseState,

    pub scrollbar_position: PageOverviewPosition,
    pub scrollbar_proportion: f64,
    pub crop_weight: f64, // 0 = no cropping and full page visible, 1size = fully cropped

    pub page_number: PageNum,
    pub page_position: f64,
    pub text_viewer_size: Size,
    pub scroll_direction: Axis,
    pub page_image_cache: Rc<RefCell<BTreeMap<PageNum, PietImage>>>,

    pub preferences: Preferences,
    pub history: Vector<PageNum>,

    //    pub page_image_cache: Rc<RefCell<BTreeMap<i32, PietImage>>>,

    // window_id: Option<WindowId>,
    //   pub doubleclick_action: DoubleClickAction,

    // on linux there's a seemingly random-positioned mouse move event when a context menu is closed, ignore it for now
    pub ignore_next_mouse_move: bool,

    pub overview_selected_page: PageNum,
    pub mouse_over_hyperlink: Option<(PageNum, String)>,
    pub scrollbar_layout: ScrollbarLayout,

    // unused?
    pub scrollbar_size: Size,
    
}

impl PdfViewState {
    pub fn new(
        docu_idx: usize,
        document: Document,
        document_info: DocumentInfo,
        preferences: Preferences,
    ) -> Self {
        let most_recent_page = document_info.most_recent_page;
        PdfViewState {
            docu_idx,
            document,
            document_info,
            scrollbar_layout: preferences.scrollbar_layout,
            preferences,
            scrollbar_position: PageOverviewPosition::East,
            scrollbar_proportion: 0.8,
            crop_weight: 1., // 0 = no cropping and full page visible, 1 = fully cropped
            page_number: most_recent_page,
            page_position: 0.5,
            scroll_direction: Axis::Horizontal,

            text_viewer_size: Size::new(100., 100.), // need to know this here so pages an be sized while scrolling, so the overview panel can tell the main page view to scroll
            page_image_cache: Rc::<RefCell<BTreeMap<PageNum, PietImage>>>::new(RefCell::new(
                BTreeMap::<PageNum, PietImage>::new(),
            )),
            //  doubleclick_action,
            ignore_next_mouse_move: false,

            overview_selected_page: most_recent_page,
            history: Vector::<PageNum>::new(),

            mouse_state: MouseState::Undragged,
            mouse_over_hyperlink: None,

            scrollbar_size: Size::ZERO,
        }
    }

    pub fn from_preexisting(old: &PdfViewState) -> Self {
        PdfViewState {
            //docu_idx:    old.docu_idx,
            document: old.document.clone(),
            document_info: old.document_info.clone(),
            preferences: old.preferences.clone(),

            //scrollbar_position: old.scrollbar_position,
            //crop_weight:      old.crop_weight,
            text_viewer_size: Size::new(100., 100.),
            //page_number:      old.page_number,
            //page_position:    old.page_position,

            //scroll_direction: old.scroll_direction,
            page_image_cache: old.page_image_cache.clone(),
            //ignore_next_mouse_move: false,
            //overview_selected_page: old.overview_selected_page,
            history: old.history.clone(),

            // reverse_bookmarks: old.reverse_bookmarks,
            mouse_state: MouseState::Undragged,

            mouse_over_hyperlink: None,
            ..*old
        }
    }

    pub fn visible_normalized_crop_margins(&mut self, page_number: PageNum) -> Rect {
        if self.crop_weight == 0. {
            UNIT_SQUARE
        } else {
            self.document_info
                .page_margins_in_normalized_coords(page_number)
        }
    }

    // 'visual_normalized_page_position' means 0 is left/top of what's visible on screen, 1 is right/bottom
    // this may differ from the 'page position' if the view is cropped
    pub fn set_visible_scroll_position(
        &mut self,
        window_id: WindowId,
        page_number: PageNum,
        visual_normalized_page_position: Option<f64>,
    ) {
        if let Some(pos) = visual_normalized_page_position {
            let crop_margins = self.visible_normalized_crop_margins(page_number);
            let (min, max) = self.scroll_direction.major_span(crop_margins);
            self.page_position = min + pos * (max - min);
        }
        self.page_number = page_number;
        // self.document
        //     .current_page_number_in_window_id
        //     .insert(window_id, page_number);
        self.document
            .rcurrent_page_number_in_window_id
            .borrow_mut()
            .insert(window_id, page_number);
        self.mouse_over_hyperlink = None;
    }

    // todo: add columns
    // the plan with complicated multi-column layouts is that always if you can see the start of a page you can also see the end of the previous one without scrolling, even if the window is too small to display entire pages beginning-to-end at once
    // that is: page start positions may not always be neatly aligned
    pub fn layout_pages(
        &self,
        viewport_size_minor_axis: f64,
        viewport_midline_offset: f64,
        viewport_size_major_axis_before_midline: f64, //todo: only one of these is needed at a time?
        viewport_size_major_axis_after_midline: f64,
        page_number: PageNum,
        page_position: f64,
        crop_weight: f64,
        required_page_numbers_range: Option<(PageNum, PageNum)>,
        scroll_direction: Axis,
    ) -> BTreeMap<PageNum, Rect> {
        let mut results = BTreeMap::new();

        let scale = 1.;

        // first page in centre, then work back and forwards
        let viewport_minor = viewport_size_minor_axis;

        let crop_rect = self
            .document_info
            .page_margins_in_normalized_coords(page_number);
        let (crop_major_min, crop_major_max) = scroll_direction.major_span(crop_rect);

        let visible_crop = lerp_rect(&UNIT_SQUARE, &crop_rect, crop_weight);
        let (visible_min, visible_max) = scroll_direction.major_span(visible_crop);

        let page_position = if page_position < visible_min || page_position > visible_max {
            if self.in_reading_mode() {
                // clamp it if we're fully cropped (probably page margins are being changed in another window)
                f64::min(crop_major_max, f64::max(crop_major_min, page_position))
            } else {
                // shift it so when moving out of crop edit mode on a window centred out of bounds the window doesn't just snap to a page boundary if one page is more over it
                visible_min + (visible_max - visible_min) * page_position
            }
        } else {
            page_position
        };

        let mut page_num = page_number;
        let page_rect = self.get_visible_page_size_in_screen_units(
            page_num,
            crop_weight,
            viewport_minor * scale,
        );

        let page_minor = scroll_direction.minor(page_rect);
        let page_major = scroll_direction.major(page_rect);

        let minor_min = (viewport_minor - page_minor) / 2.;
        let minor_max = minor_min + page_minor;
        let mut major_min = viewport_midline_offset
            - page_major * (page_position - visible_min) / (visible_max - visible_min);
        let remember_min = major_min;
        let mut major_max = major_min + page_major;

        let rect = match scroll_direction {
            Axis::Horizontal => Rect::new(major_min, minor_min, major_max, minor_max),
            Axis::Vertical => Rect::new(minor_min, major_min, minor_max, major_max),
        };

        results.insert(page_num, rect);

        let mut min_page: PageNum = page_number;
        let mut max_page: PageNum = page_number;
        if let Some((min, max)) = required_page_numbers_range {
            min_page = min;
            max_page = max;
        }

        while page_num + 1 < self.document_info.page_count
            && (major_max < viewport_midline_offset + viewport_size_major_axis_after_midline
                || page_num < max_page)
        {
            page_num += 1;
            let page_rect = self.get_visible_page_size_in_screen_units(
                page_num,
                crop_weight,
                viewport_minor * scale,
            );

            let page_major = scroll_direction.major(page_rect);

            let minor_min = (viewport_minor - page_minor) / 2.;
            let minor_max = minor_min + page_minor;
            major_min = major_max;
            major_max = major_min + page_major;

            let rect = match scroll_direction {
                Axis::Horizontal => Rect {
                    x0: major_min,
                    x1: major_max,
                    y0: minor_min,
                    y1: minor_max,
                },
                // {Rect {x0: minor_min, x1: minor_max, y0: major_min, y1: major_max}},
                Axis::Vertical => Rect {
                    x0: minor_min,
                    x1: minor_max,
                    y0: major_min,
                    y1: major_max,
                },
            };

            results.insert(page_num, rect);
        }
        page_num = page_number;

        major_min = remember_min;
        while 0 < page_num
            && (major_min > viewport_midline_offset - viewport_size_major_axis_before_midline
                || page_num > min_page)
        {
            page_num -= 1;
            let page_rect = self.get_visible_page_size_in_screen_units(
                page_num,
                crop_weight,
                viewport_minor * scale,
            );

            let page_major = scroll_direction.major(page_rect);

            let minor_min = (viewport_minor - page_minor) / 2.;
            let minor_max = minor_min + page_minor;
            major_max = major_min;
            major_min = major_max - page_major;

            let rect = match scroll_direction {
                Axis::Horizontal =>
                // {Rect {x0: minor_min, x1: minor_max, y0: major_min, y1: major_max}},
                {
                    Rect {
                        x0: major_min,
                        x1: major_max,
                        y0: minor_min,
                        y1: minor_max,
                    }
                }
                Axis::Vertical => Rect {
                    x0: minor_min,
                    x1: minor_max,
                    y0: major_min,
                    y1: major_max,
                },
            };

            results.insert(page_num, rect);
        }

        results
    }

    pub fn layout_pages_within_visible_window(
        &self,
        viewport_size: Size,
        crop_weight: f64,
        required_page_numbers_range: Option<(PageNum, PageNum)>, // generate page positions even for pages which aren't visible at this magnification / crop weight, so page resizing animations work
    ) -> BTreeMap<PageNum, Rect> {
        self.layout_pages(
            self.scroll_direction.minor(viewport_size),
            self.scroll_direction.major(viewport_size) / 2.,
            self.scroll_direction.major(viewport_size) / 2.,
            self.scroll_direction.major(viewport_size) / 2.,
            self.page_number,
            self.page_position,
            crop_weight,
            required_page_numbers_range,
            self.scroll_direction,
        )
    }

    pub fn scroll_by(
        &mut self,
        window_id: WindowId,
        distance: f64,
        start_page_number: PageNum,
        start_page_position: f64,
    ) -> BTreeMap<PageNum, Rect> {
        // look through page position rectangles for the new window centre, if there aren't enough then lay out more pages

        let new_center_position =
            distance + self.scroll_direction.major(self.text_viewer_size) / 2.;

        let mut required_viewport_coverage_before_midline = 0.;
        let mut required_viewport_coverage_following_midline = distance;
        if distance < 0. {
            required_viewport_coverage_before_midline = -distance;
            required_viewport_coverage_following_midline = 0.;
        }

        let layout = self.layout_pages(
            self.scroll_direction.minor(self.text_viewer_size),
            self.scroll_direction.major(self.text_viewer_size) / 2.,
            required_viewport_coverage_before_midline,
            required_viewport_coverage_following_midline,
            start_page_number,
            start_page_position,
            self.crop_weight,
            None,
            self.scroll_direction,
        );

        let mut screen_rect = layout.get(&start_page_number).unwrap();
        let (mut min, mut max) = self.scroll_direction.major_span(*screen_rect);
        let mut page_number = start_page_number;

        if distance > 0. {
            while max < new_center_position {
                if page_number + 1 >= self.document_info.page_count {
                    println!("hit end of document");
                    self.set_visible_scroll_position(window_id, page_number, Some(1.));
                    return layout;
                }

                page_number += 1;
                screen_rect = layout.get(&page_number).unwrap();
                let (nmin, nmax) = self.scroll_direction.major_span(*screen_rect);
                min = nmin;
                max = nmax;
            }
        } else {
            // scrolling backwards because `distance` is negative
            while min > new_center_position {
                if page_number == 0 {
                    println!("hit start of document");
                    self.set_visible_scroll_position(window_id, page_number, Some(0.));
                    return layout;
                }

                page_number -= 1;
                println!("getting page {}", page_number);
                screen_rect = layout.get(&page_number).unwrap();
                let (nmin, nmax) = self.scroll_direction.major_span(*screen_rect);
                min = nmin;
                max = nmax;
            }
        }
        //
        if self.overview_selected_page == self.page_number {
            self.overview_selected_page = page_number;
        }
        self.set_visible_scroll_position(
            window_id,
            page_number,
            Some((new_center_position - min) / (max - min)),
        );

        layout
    }

    pub fn get_visible_page_size_in_screen_units(
        &self,
        page_number: PageNum,
        crop_weight: f64,
        minor_axis_page_size: f64,
    ) -> Size {
        let crop_rect = self
            .document_info
            .weighted_page_margins_in_normalized_coords(page_number, crop_weight);

        let mut page_size = self.document.get_page_size_in_points(page_number);
        page_size.width *= crop_rect.width();
        page_size.height *= crop_rect.height();

        match self.scroll_direction {
            Axis::Horizontal => Size {
                width: (page_size.width / page_size.height) * minor_axis_page_size,
                height: minor_axis_page_size,
            },
            Axis::Vertical => Size {
                width: minor_axis_page_size,
                height: (page_size.height / page_size.width) * minor_axis_page_size,
            },
        }
    }

    pub fn get_page_pixmap(&self, page_number: PageNum, size: Size) -> Pixmap {
        let page = self.document.load_page(page_number);
        let bounds = page.bounds().expect("Unable to get page bounds");
        let crop_rect = self
            .document_info
            .page_margins_in_normalized_coords(page_number);

        let screen_units_per_page_point = (self.scroll_direction.minor(size)) as f32
            / match self.scroll_direction {
                Axis::Horizontal => (crop_rect.height() as f32 * bounds.height()),
                Axis::Vertical => (crop_rect.width() as f32 * bounds.width()),
            };

        // tofix: find how to look up actual pixel density -- https://docs.rs/druid/0.7.0/druid/struct.Scale.html#converting-with-scale looks like it, but how to get a scale struct to start with?
        #[allow(unused_mut)]
        let mut high_pixel_density_scaling = 1.;
        #[cfg(target_os = "macos")]
        {
            let high_pixel_density_scaling = 2.;
        }

        let matrix = Matrix::new_scale(
            high_pixel_density_scaling * screen_units_per_page_point,
            high_pixel_density_scaling * screen_units_per_page_point,
        );

        let mut pixmap = page
            .to_pixmap(&matrix, &Colorspace::device_rgb(), 0.0, true)
            .expect("Unable to render PDF page");

        let i = self.preferences.brightness_inversion_amount;

        let process = |p: u8| -> u8 {
            if i > 0.5 {
                255 - (i * p as f64) as u8
            } else {
                ((1. - i) * p as f64) as u8
            }
        };

        // colour inversions
        let w = pixmap.width() as usize;
        let h = pixmap.height() as usize;
        let pxls = pixmap.samples_mut();
        for y in 0..h {
            for x in 0..w {
                let p = 3 * (x + y * w);
                pxls[p] = process(pxls[p]);
                pxls[p + 1] = process(pxls[p + 1]);
                pxls[p + 2] = process(pxls[p + 2]);
            }
        }

        if let Some(rects) = self
            .document_info
            .color_inversion_rectangles
            .get(&page_number)
        {
            for r in rects {
                let pxls = pixmap.samples_mut();
                let min_x = f64::round(w as f64 * r.min_x()) as usize;
                let max_x = f64::round(w as f64 * r.max_x()) as usize;
                let min_y = f64::round(h as f64 * r.min_y()) as usize;
                let max_y = f64::round(h as f64 * r.max_y()) as usize;

                for y in min_y..usize::min(max_y, h) {
                    for x in min_x..usize::min(max_x, w) {
                        let p = (3 * (x + y * w)) as usize;
                        pxls[p] = 255 - pxls[p];
                        pxls[p + 1] = 255 - pxls[p + 1];
                        pxls[p + 2] = 255 - pxls[p + 2];
                    }
                }
            }
        }

        println!("Rendered page {} at {} x {}", page_number, w, h);

        pixmap
    }

    pub fn get_page_image(
        &self,
        page_number: PageNum,
        size: Size,
        ctx: &mut PaintCtx,
    ) -> PietImage {
        let pixmap = self.get_page_pixmap(page_number, size);

        ctx.make_image(
            pixmap.width() as usize,
            pixmap.height() as usize,
            &pixmap.samples(),
            ImageFormat::Rgb,
        )
        .expect("Unable to make druid image from mupdf pixmap")
    }

    pub fn select_page(&mut self, page: usize) {
        self.overview_selected_page = page;
        self.document_info.most_recent_page = page;
        self.document.doc_info_changed = true;
    }

    pub fn in_reading_mode(&self) -> bool {
        self.crop_weight >= 1.
    }

    pub fn check_for_hyperlinks(&mut self, ctx: &mut EventCtx, page_number: PageNum, pos: Point) {
        let links_on_page = match self.document.hyperlinks.get(&page_number) {
            Some(map_entry) => map_entry.clone(),
            None => {
                let page = self.document.load_page(page_number);

                let Size { width, height } = self.document.get_page_size_in_points(page_number);

                let mut acc = Vector::<Hyperlink>::new();
                if let Ok(links) = page.links() {
                    for l in links {
                        let bounds = l.bounds;
                        acc.push_back(Hyperlink {
                            link: (l.page as usize, l.uri.clone()),
                            bounds: Rect::new(
                                bounds.x0 as f64 / width,
                                bounds.y0 as f64 / height,
                                bounds.x1 as f64 / width,
                                bounds.y1 as f64 / height,
                            ),
                        });
                    }
                }
                if ! acc.is_empty() {
                    self.document
                        .hyperlinks
                        .insert(page_number, Some(acc.clone()));
                    Some(acc)
                } else {
                    self.document.hyperlinks.insert(page_number, None);
                    None
                }
            }
        };

        if let Some(links) = links_on_page {
            let mut mouse_over = None;
            for l in links {
                // println!("{} {}", l.bounds, pos);
                if l.bounds.contains(pos) {
                    mouse_over = Some(l.link);
                    break;
                }
            }

            if mouse_over != None {
                ctx.set_cursor(&druid::Cursor::Crosshair);
            } else {
                ctx.set_cursor(&druid::Cursor::Arrow);
            }
            self.mouse_over_hyperlink = mouse_over;
        } else {
            ctx.set_cursor(&druid::Cursor::Arrow);
            self.mouse_over_hyperlink = None;
        }
    }

    fn page_number_or_link_target(&self) -> PageNum {
        if let Some((hyperlink_to_page_number, _)) = self.mouse_over_hyperlink {
            if hyperlink_to_page_number > 0 {
                return hyperlink_to_page_number;
            }
        }
        self.page_number
    }

    pub fn show_page(&mut self, window_id: WindowId, page_number: PageNum) {
        let page_number = PageNum::min(page_number, self.document_info.page_count - 1);
        if self.page_number == self.overview_selected_page {
            self.select_page(page_number);
        }
        self.set_visible_scroll_position(window_id, page_number, None);
    }

    // todo: prefer vertical scrolling unless at least two full pages can be visible horizontally
    // todo: multi-columns / -rows
    pub fn adjust_zoom(&mut self, ctx: &mut EventCtx, desired_scaling: f64) {
        let page_points_size = self.document.get_page_size_in_points(self.page_number);
        let page_size = self
            .document_info
            .weighted_page_margins_in_normalized_coords(self.page_number, self.crop_weight);

        let page = Size::new(
            page_points_size.width * page_size.width(),
            page_points_size.height * page_size.height(),
        );

        let window = ctx.size();

        let current_width;
        let current_height;
        match self.scroll_direction {
            Axis::Vertical => {
                match self.scrollbar_position {
                    PageOverviewPosition::East | PageOverviewPosition::West => {
                        current_width = window.width * self.scrollbar_proportion
                    }
                    _ => current_width = window.width,
                }
                current_height = current_width * page.height / page.width;
            }
            Axis::Horizontal => {
                match self.scrollbar_position {
                    PageOverviewPosition::North | PageOverviewPosition::South => {
                        current_height = window.height * self.scrollbar_proportion
                    }
                    _ => current_height = window.height,
                }
                current_width = current_height * page.width / page.height;
            }
        }

        let vert_prop_reqd = (current_width * desired_scaling) / window.width;
        let horiz_prop_reqd = (current_height * desired_scaling) / window.height;

        if vert_prop_reqd < 1. && horiz_prop_reqd < 1. {
            if horiz_prop_reqd > vert_prop_reqd {
                self.scroll_direction = Axis::Horizontal;
                self.scrollbar_position = PageOverviewPosition::South;
                self.scrollbar_proportion = horiz_prop_reqd;
            } else {
                self.scroll_direction = Axis::Vertical;
                self.scrollbar_position = PageOverviewPosition::East;
                self.scrollbar_proportion = vert_prop_reqd;
            }
        } else if vert_prop_reqd >= 0.99 && horiz_prop_reqd >= 0.99 {
            if self.scroll_direction == Axis::Vertical {
                if desired_scaling > 0.99 {
                    self.scrollbar_position = PageOverviewPosition::South;
                } else {
                    self.scrollbar_position = PageOverviewPosition::East;
                }
            } else if desired_scaling > 0.99 {
                self.scrollbar_position = PageOverviewPosition::East;
            } else {
                self.scrollbar_position = PageOverviewPosition::South;
            }

            // if horiz_prop_reqd > vert_prop_reqd {
            //     self.scroll_direction = Axis::Horizontal;
            //     self.scrollbar_position = PageOverviewPosition::East;
            //     //self.scrollbar_proportion = horiz_prop_reqd;
            // } else {
            //     self.scroll_direction = Axis::Vertical;
            //     self.scrollbar_position = PageOverviewPosition::South;
            //     //self.scrollbar_proportion = vert_prop_reqd;
            // }
        } else {
            if vert_prop_reqd < 0.99 {
                self.scroll_direction = Axis::Vertical;
                self.scrollbar_position = PageOverviewPosition::East;
                self.scrollbar_proportion = vert_prop_reqd;
            }
            if horiz_prop_reqd < 0.99 {
                self.scroll_direction = Axis::Horizontal;
                self.scrollbar_position = PageOverviewPosition::South;
                self.scrollbar_proportion = horiz_prop_reqd;
            }
        }


        if desired_scaling > 1. {
            // debug mode is slow at drawing pages, release mode very fast
            #[cfg(not(debug_assertions))]
            self.page_image_cache.borrow_mut().clear();
        }
    }
}

use crate::Hyperlink;

pub struct DocTransfer;

impl ScopeTransfer for DocTransfer {
    type In = AppState;

    type State = PdfViewState;

    fn read_input(&self, my_state: &mut Self::State, external: &Self::In) {
        my_state.document = external
            .loaded_documents
            .get(my_state.docu_idx)
            .unwrap()
            .clone();
        my_state.document_info = external
            .all_local_documents_info
            .get(&my_state.document.fingerprint)
            .unwrap()
            .clone();
        my_state.preferences = external.preferences.clone();
    }

    fn write_back_input(&self, my_state: &Self::State, external: &mut Self::In) {
        external
            .loaded_documents
            .set(my_state.docu_idx, my_state.document.clone()); // .clone() ?
        external.all_local_documents_info.insert(
            my_state.document.fingerprint.clone(),
            my_state.document_info.clone(),
        );
    }
}

// document view
// preferences
// recent files
// open file
// notes window

pub fn make_pdf_view_window(
    app_state: &mut AppState,
    doc_idx: usize,
    old_view: Option<PdfViewState>,
) -> WindowDesc<AppState> // impl Widget<AppState>
{
    use PageOverviewPosition::*;

    let scope = Scope::from_function(
        move |data: AppState| match &old_view {
            Some(old_view_state) => PdfViewState::from_preexisting(&old_view_state),
            Option::None => PdfViewState::new(
                doc_idx,
                data.loaded_documents.get(doc_idx).unwrap().clone(),
                data.all_local_documents_info
                    .get(&data.loaded_documents.get(doc_idx).unwrap().fingerprint)
                    .unwrap()
                    .clone(),
                data.preferences.clone(),
            ),
        },
        DocTransfer,
        Container::new(
            ViewSwitcher::new(
                |data: &PdfViewState, _env| (data.scrollbar_position, data.scrollbar_proportion),
                |selector, data: &PdfViewState, _env| match selector {
                    (North, proportion) => Box::new(
                        Split::rows(
                            ScrollbarWidget::with_layout_and_length(
                                data.scrollbar_layout,
                                data.document_info.page_count,
                            ),
                            PdfTextWidget::new(),
                        )
                        //HilbertCurve::new())
                        .split_point(1. - *proportion)
                        .draggable(true)
                        .solid_bar(true),
                    ),
                    (South, proportion) => {
                        Box::new(
                            Split::rows(
                                PdfTextWidget::new(),
                                // Split::columns(
                                // ContentsTree::default(),
                                ScrollbarWidget::with_layout_and_length(
                                    data.scrollbar_layout,
                                    data.document_info.page_count,
                                ),
                                //                 )
                                //     .draggable(true)
                                //     .solid_bar(true)
                                //     .split_point(0.2),
                            )
                            //HilbertCurve::new())
                            .split_point(*proportion)
                            .draggable(true)
                            .solid_bar(true),
                        )
                    }
                    (East, proportion) => Box::new(
                        Split::columns(
                            PdfTextWidget::new(),
                            // Split::rows(
                            //     ContentsTree::default(),
                            ScrollbarWidget::with_layout_and_length(
                                data.scrollbar_layout,
                                data.document_info.page_count,
                            ),
                            //             )
                            // .draggable(true)
                            // .solid_bar(true)
                            // .split_point(0.8),
                            //HilbertCurve::new())
                        )
                        .split_point(*proportion)
                        .draggable(true)
                        .solid_bar(true),
                    ),
                    (West, proportion) => Box::new(
                        Split::columns(
                            ScrollbarWidget::with_layout_and_length(
                                data.scrollbar_layout,
                                data.document_info.page_count,
                            ),
                            PdfTextWidget::new(),
                            //HilbertCurve::new())
                        )
                        .split_point(1. - *proportion)
                        .draggable(true)
                        .solid_bar(true),
                    ),
                    (Nowhere, _) => Box::new(PdfTextWidget::new()),
                },
            )
            .controller(PdfWindowController), // Box::new(Label::new("Things"))
                                              // )
        ),
    );

    // fn title_maker(data: &AppState, doc_id: usize, win_id: WindowId) -> String {

    // }

    let new_win = WindowDesc::new(scope);
    let win_id = new_win.id;
    let info = &app_state
        .all_local_documents_info
        .get(&app_state.loaded_documents[doc_idx].fingerprint)
        .unwrap();
    let page = info.most_recent_page;
    // app_state.loaded_documents[doc_idx].current_page_number_in_window_id
    //                             .insert(win_id, page);
    app_state
        .rcurrent_page_number_in_window_id
        .borrow_mut()
        .insert(win_id, page);

    let page_count = info.page_count;
    let user_facing_path = app_state.loaded_documents[doc_idx].user_facing_path.clone();

    new_win.window_size((1024., 1024.)).title(
        move |data: &AppState, _env: &Env| {
            format!(
                "[{}/{}] {}",
                // data.loaded_documents[doc_id].current_page_number_in_window_id
                //                       .get(&win_id)
                //                       .unwrap_or(&0),
                data.rcurrent_page_number_in_window_id
                    .borrow()
                    .get(&win_id)
                    .unwrap_or(&42069),
                page_count,
                user_facing_path
            )
        }, //        title_maker(data, doc_idx, win_id)
    )

    //.lens(AppState::documents.index(doc_idx))
}

use crate::CHECK_FOR_WINDOWS_TO_OPEN;

struct PdfWindowController;

impl<W: Widget<PdfViewState>> Controller<PdfViewState, W> for PdfWindowController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut PdfViewState,
        env: &Env,
    ) {
        match event {
            Event::WindowConnected => {
                ctx.submit_command(CHECK_FOR_WINDOWS_TO_OPEN);
                ctx.request_focus();
            }
            Event::Command(cmd) => {
                //                } else
                if cmd.is(REFRESH_PAGE_IMAGES) {
                    data.page_image_cache.borrow_mut().clear();
                    ctx.request_paint();
                } else if cmd.is(REPOSITION_OVERVIEW) {
                    data.scrollbar_position = data.scrollbar_position.next();
                } else if cmd.is(NEW_VIEW) {
                    ctx.submit_command(
                        NEW_VIEW_WITH_PARENT
                            .with(data.clone())
                            .to(druid::Target::Global),
                    );
                } else if let Some(page_number) = cmd.get(TOGGLE_EVEN_ODD_PAGE_DISTINCTION) {
                    data.document_info
                        .toggle_even_odd_page_distinction(*page_number);
                } else if let Some(page_number) = cmd.get(CUSTOMIZE_PAGE_CROP) {
                    data.document_info.toggle_custom_margins(*page_number);
                } else if let Some(page_number) = cmd.get(REMOVE_COLOR_INVERSION_RECTANGLE) {
                    if let Some(rects) = data
                        .document_info
                        .color_inversion_rectangles
                        .get_mut(page_number)
                    {
                        rects.pop_back();
                        data.page_image_cache.borrow_mut().remove(page_number);
                    }
                } else {
                    child.event(ctx, event, data, env);
                }
            }

            Event::KeyDown(e) => {
                if e.key == Key::F5 || e.mods.ctrl() && e.key == Key::Character("r".to_string()) {
                    ctx.submit_command(REFRESH_PAGE_IMAGES);
                } else if e.mods.ctrl() {
                    // can't use druid::keyboard_types::Code::KeyN etc because i'm not typing qwerty
                    if let Key::Character(k) = &e.key {
                        match k.as_str() {
                            "+" | "=" => data.adjust_zoom(ctx, 1.05),
                            "-" | "_" => data.adjust_zoom(ctx, 0.95),
                            "o" => {
                                let pdf = FileSpec::new("PDF file", &["pdf"]);
                                let open_dialogue_options =
                                    FileDialogOptions::new().allowed_types(vec![pdf])
                                                            .default_type(pdf)
                                                            //.default_name(default_save_name)
                                                            //.name_label("Target")
                                                            .force_starting_directory(data.document
                                                                                          .filepath
                                                                                          .clone())
                                                            .title("Choose a PDF file to open")
                                                            .button_text("Open");
                                ctx.submit_command(Command::new(druid::commands::SHOW_OPEN_PANEL,
                                                                open_dialogue_options,
                                                                Target::Auto));
                            },
                            "i" =>
                                ctx.submit_command(crate::pdf_view::START_INVERSION_AREA_SELECTION),
                            "e" =>
                                ctx.submit_command(crate::pdf_view::TOGGLE_CROP_MODE),
                            "n" =>//druid::keyboard_types::Code::KeyN =>
                                ctx.submit_command(NEW_VIEW),
                            "b" => ctx.submit_command(SHOW_BOOK_INFO.with(data.docu_idx)),
                            "p" => ctx.submit_command(SHOW_PREFERENCES),
                            _ => (),
                        }
                    }
                } else if e.key == Key::Character("+".to_string())
                    || e.key == Key::Character("=".to_string())
                {
                    //                    data.zoom_in(ctx);
                    data.adjust_zoom(ctx, 1.05);
                } else if e.key == Key::Character("-".to_string())
                    || e.key == Key::Character("_".to_string())
                {
                    data.adjust_zoom(ctx, 0.95);
                } else if e.key == Key::Tab {
                    if e.mods.shift() {
                        ctx.submit_command(crate::pdf_view::SCROLL_DIRECTION_TOGGLE);
                    } else {
                        data.scrollbar_position = data.scrollbar_position.next();
                    }
                } else if e.key == Key::Character("/".to_string()) {
                    // todo - proper search function

                    let page = data.document.load_page(data.page_number);

                    let t = "type";
                    let links = page.search(&t, 100);
                    if let Ok(finds) = links {
                        for f in finds {
                            println!("found {:?}", f);
                        }
                    }
                } else if e.key == Key::Enter {
                    if data.page_number != data.overview_selected_page {
                        data.history.push_back(data.overview_selected_page);
                        data.select_page(data.page_number);
                    }
                } else if e.key == Key::ArrowLeft || e.key == Key::ArrowUp {
                    let step = if e.mods.shift() { 10 } else { 1 };
                    data.show_page(ctx.window_id(), data.page_number.saturating_sub(step));
                } else if e.key == Key::Character(",".to_string()) {
                    let page = data.page_number.saturating_sub(1);
                    data.document_info
                        .set_tags(page, data.document_info.tag_bits(data.page_number));
                    data.show_page(ctx.window_id(), page);
                } else if e.key == Key::Character(".".to_string()) {
                    data.document_info.set_tags(
                        data.page_number + 1,
                        data.document_info.tag_bits(data.page_number),
                    );
                    data.show_page(ctx.window_id(), data.page_number + 1);
                } else if e.key == Key::ArrowRight || e.key == Key::ArrowDown {
                    let step = if e.mods.shift() { 10 } else { 1 };
                    data.show_page(ctx.window_id(), data.page_number + step);
                } else if e.key == Key::Backspace {
                    if let Some(page) = data.history.pop_back() {
                        data.set_visible_scroll_position(ctx.window_id(), page, None);
                        data.select_page(page);
                    }
                } else if !e.mods.ctrl() && !e.mods.alt() {
                    let s = e.key.to_string();
                    if s == " " {
                        if let Some(s) = data
                            .document
                            .reverse_bookmarks
                            .get(&data.page_number_or_link_target())
                        {
                            data.document.doc_info_changed = true;
                            data.document_info.bookmarks.remove(s);
                            data.document
                                .generate_reverse_bookmarks(&data.document_info);
                        }
                    } else if s.len() == 1 {
                        let mut chr = s.chars();
                        if let Some(ch) = chr.next() {
                            if ch.is_numeric() {
                                if let Some(num) = char::to_digit(ch, 10) {
                                    data.document.doc_info_changed = true;
                                    let page = data.page_number_or_link_target();
                                    if num == 0 {
                                        data.document_info.clear_tags(page);
                                    } else {
                                        data.document_info.toggle_tag_bit(page, num);
                                    }
                                }
                            } else if ch.is_alphabetic() {
                                data.document.doc_info_changed = true;
                                if let Some(page_number) =
                                    data.document_info.bookmarks.get(&ch.to_string())
                                {
                                    let pn = *page_number;
                                    data.set_visible_scroll_position(ctx.window_id(), pn, None);

                                    data.history.push_back(data.overview_selected_page);
                                    data.select_page(pn);
                                } else {
                                    let page = data.page_number_or_link_target();

                                    // forget any bookmarks which were assigned to this page
                                    if let Some(s) = data.document.reverse_bookmarks.get(&page) {
                                        data.document_info.bookmarks.remove(s);
                                    }

                                    data.document_info.add_bookmark(&ch.to_string(), page);
                                    data.document
                                        .generate_reverse_bookmarks(&data.document_info);
                                }
                                ctx.request_paint();
                            } else {
                                //                                println!("Unhandled key event {:?}", e);
                            }
                        }
                    }
                } else {
                    child.event(ctx, event, data, env)
                }
            }

            Event::MouseDown(e) => {
                if e.buttons.has_x1() {
                    if let Some(page) = data.history.pop_back() {
                        data.set_visible_scroll_position(ctx.window_id(), page, None);
                        data.select_page(page);
                    }
                } else {
                    child.event(ctx, event, data, env)
                }
            }

            Event::Wheel(e) => {
                let x = e.wheel_delta.x;
                let y = e.wheel_delta.y;
                let distance = f64::signum(x + y) * f64::sqrt(x * x + y * y);
                if e.mods.ctrl() {
                    if distance < 0. {
                        data.adjust_zoom(ctx, 1.05);
                    } else {
                        data.adjust_zoom(ctx, 0.95);
                    }
                } else {
                    data.scroll_by(
                        ctx.window_id(),
                        distance,
                        data.page_number,
                        data.page_position,
                    );
                }
                child.event(ctx, event, data, env)
            }

            _ => {
                // println!("window event {:?}", event);
                child.event(ctx, event, data, env)
            }
        }
    }

    fn lifecycle(
        &mut self,
        child: &mut W,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &PdfViewState,
        env: &Env,
    ) {
        match event {
            LifeCycle::HotChanged(_now) => {
                if data.document.doc_info_changed && !ctx.is_hot() {
                    // save on focus lost
                    ctx.submit_command(
                        SAVE_DOCUMENT_INFO.with(data.document_info.fingerprint.clone()),
                    );
                }
                child.lifecycle(ctx, event, data, env)
            }
            _ => child.lifecycle(ctx, event, data, env),
        }
    }
}

use std::convert::TryInto;

pub fn make_context_menu<T: Data>(data: &mut PdfViewState, page_number: PageNum) -> MenuDesc<T> {
    let scroll_vert = LocalizedString::new("Vertical page scroll direction");
    let scroll_horiz = LocalizedString::new("Horizontal page scroll direction");

    let start_crop = LocalizedString::new("Edit page crop margins");
    let finish_crop = LocalizedString::new("Finish editing page crop margins");

    let new_view = LocalizedString::new("New view window");

    MenuDesc::empty()
        .append(MenuItem::new(new_view, NEW_VIEW).hotkey(SysMods::Cmd, "n"))
        .append(
            MenuItem::new(
                if data.crop_weight == 0. {
                    finish_crop
                } else {
                    start_crop
                },
                crate::pdf_view::TOGGLE_CROP_MODE,
            )
            .hotkey(SysMods::Cmd, "e"),
        )
        .append_if(
            MenuItem::new(
                LocalizedString::new("Use a unique custom margin for this page"),
                CUSTOMIZE_PAGE_CROP.with(page_number),
            )
            .selected_if(|| data.document_info.has_custom_margins(page_number)),
            || !data.in_reading_mode(),
        )
        .append_if(
            MenuItem::new(
                LocalizedString::new("Use different margins for even- vs odd-numbered pages"),
                TOGGLE_EVEN_ODD_PAGE_DISTINCTION.with(page_number),
            )
            .selected_if(|| data.document_info.are_even_and_odd_distinguished()),
            || !data.in_reading_mode(),
        )
        .append(
            MenuItem::new(
                LocalizedString::new("Reposition 'scrollbar'"),
                REPOSITION_OVERVIEW,
            )
            .hotkey(SysMods::None, Key::Tab),
        )
        .append(
            MenuItem::new(
                if data.scroll_direction == Axis::Vertical {
                    scroll_horiz
                } else {
                    scroll_vert
                },
                SCROLL_DIRECTION_TOGGLE,
            )
            .hotkey(SysMods::Shift, Key::Tab),
        )
        .append(
            MenuItem::new(
                LocalizedString::new("Invert the colors of part of the page"),
                START_INVERSION_AREA_SELECTION,
            )
            .hotkey(SysMods::Cmd, "i"),
        )
        .append_if(
            MenuItem::new(
                LocalizedString::new("Remove the most recent color inversion"),
                REMOVE_COLOR_INVERSION_RECTANGLE.with(page_number),
            ),
            || match data
                .document_info
                .color_inversion_rectangles
                .get(&page_number)
            {
                Some(rects) => ! rects.is_empty(),
                None => false,
            },
        )
        .append(
            MenuItem::new(LocalizedString::new("Refresh window"), REFRESH_PAGE_IMAGES)
                .hotkey(SysMods::None, Key::F5),
        )
        .append(
            MenuItem::new(
                LocalizedString::new("Open another book..."),
                SHOW_BOOK_INFO.with(data.docu_idx),
            )
            .hotkey(SysMods::Cmd, "b"),
        )
        .append(
            MenuItem::new(LocalizedString::new("Preferences..."), SHOW_PREFERENCES)
                .hotkey(SysMods::Cmd, "p"),
        )
}
