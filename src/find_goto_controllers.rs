use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, Command, FileDialogOptions, FileSpec, FontDescriptor, FontStyle,
    FontWeight, Handled, Lens, LocalizedString, Menu, MenuItem, MouseButton, MouseEvent, Point,
    Rect, Selector, SysMods, Target, TextLayout, Vec2, WindowDesc, WindowId,
};

use druid::widget::{
    Align, Axis, Container, Controller, ControllerHost, Flex, Label, LineBreaking, Padding,
    Painter, RadioGroup, Scope, ScopeTransfer, SizedBox, Slider, Split, TextBox, ViewSwitcher,
    WidgetExt,
};

use druid::keyboard_types::Key;

use crate::pdf_view::{PdfViewState, WindowMode, SET_WINDOW_MODE};

pub const FOCUS_FIND_TEXTBOX: Selector = Selector::new("focus-find-textbox");
pub const START_SEARCH: Selector = Selector::new("start-search");
pub const START_GOTO: Selector = Selector::new("start-goto");

struct FindController;
struct GotoController;
struct OffsetController;

pub fn make_find_ui() -> impl Widget<PdfViewState> {
    SizedBox::new(
        Flex::row()
            .with_child(
                Label::new("Find exact phrase, case insensitively: ").controller(FindController),
            )
            .with_child(
                TextBox::new()
                    .lens(PdfViewState::find_goal)
                    .controller(FindController),
            ),
    )
    .height(50.)
}

pub fn make_goto_ui() -> impl Widget<PdfViewState> {
    SizedBox::new(
        Flex::row()
            .with_child(Label::new("Go to page"))
            .with_default_spacer()
            .with_child(
                TextBox::new()
                    .lens(PdfViewState::goto_page)
                    .controller(GotoController),
            )
            .with_default_spacer()
            .with_child(Label::new(
                "using a printed page number -> file page number offset of",
            ))
            .with_default_spacer()
            .with_child(
                TextBox::new()
                    .lens(PdfViewState::goto_offset)
                    .controller(OffsetController),
            )
            .with_default_spacer()
            .with_child(Label::new(
                "[= page number shown in window title bar for text page #1]",
            )),
    )
    .height(50.)
}

use crate::pdf_text_widget::SHOW_GIVEN_PAGE;

impl<W: Widget<PdfViewState>> Controller<PdfViewState, W> for GotoController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut PdfViewState,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) => {
                if cmd.is(START_GOTO) {
                    ctx.request_focus();
                    ctx.set_handled();
                } else {
                    child.event(ctx, event, data, env);
                }
            }
            Event::KeyDown(e) => {
                if e.key == Key::Escape
                    || e.key == Key::Enter
                    || e.key == Key::Character(" ".to_string())
                {
                    ctx.resign_focus();
                    data.window_mode = WindowMode::Normal;
                    ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Normal));
                    ctx.set_handled();
                } else if e.mods.ctrl() && e.key == Key::Character("f".to_string())
                    || e.key == Key::Character("/".to_string())
                    || e.key == Key::F3
                {
                    ctx.resign_focus();
                    data.window_mode = WindowMode::Find;
                    ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Find));
                    ctx.set_handled();
                } else {
                    child.event(ctx, event, data, env);
                }
            }
            _ => {
                child.event(ctx, event, data, env);
                let old_page = data.page_number as i32 + 1 - data.document_info.page_offset;
                let new_page = data.goto_page.parse::<i32>().unwrap_or(old_page);

                let p = new_page - 1 + data.document_info.page_offset;
                if new_page != old_page && p >= 0 && (p as usize) < data.document_info.page_count {
                    data.page_number = p as usize;
                }

                if p >= 0 && (p as usize) < data.document_info.page_count {
                    data.set_visible_scroll_position(ctx.window_id(), p as PageNum, None);
                    data.select_page(p as PageNum);
                }
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
            LifeCycle::WidgetAdded => {
                ctx.submit_command(START_GOTO);
                child.lifecycle(ctx, event, data, env);
            }
            _ => child.lifecycle(ctx, event, data, env),
        }
    }

    // fn update(&mut self,
    //           child: &mut W,
    //           ctx: &mut UpdateCtx<'_, '_>,
    //           old_data: &PdfViewState,
    //           data: &PdfViewState,
    //           env: &Env) {
    //     let page = data.goto_page.parse::<i32>().unwrap_or(data.page_number as i32);
    //     let offset = data.goto_offset.parse::<i32>().unwrap_or(data.document_info.page_offset);
    //     let p = page + offset;
    //     if p >= 0 && p < data.document_info.page_count.try_into().unwrap() && p as usize != data.page_number {
    //         ctx.submit_command(
    //             SHOW_GIVEN_PAGE
    //                 .with(p as PageNum)
    //                 .to(druid::Target::Window(ctx.window_id())),
    //         );
    //     }
    // }
}

use crate::PageNum;
use std::convert::TryInto;

impl<W: Widget<PdfViewState>> Controller<PdfViewState, W> for OffsetController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut PdfViewState,
        env: &Env,
    ) {
        child.event(ctx, event, data, env);

        if let Event::KeyDown(e) = event {
            if e.key == Key::Escape
                || e.key == Key::Enter
                || e.key == Key::Character(" ".to_string())
            {
                ctx.resign_focus();
                data.window_mode = WindowMode::Normal;
                ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Normal));
                ctx.set_handled();
            } else if e.mods.ctrl() && e.key == Key::Character("f".to_string())
                || e.key == Key::Character("/".to_string())
                || e.key == Key::F3
            {
                ctx.resign_focus();
                data.window_mode = WindowMode::Find;
                ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Find));
                ctx.set_handled();
            } else {
                child.event(ctx, event, data, env);
            }
        } else {
            let new_offset = data
                .goto_offset
                .parse::<i32>()
                .unwrap_or(data.document_info.page_offset);
            if new_offset != data.document_info.page_offset {
                data.document.doc_info_changed = true;
                data.document_info.page_offset = new_offset;
                let p = (data.page_number as i32 - 1 + new_offset);
                if p >= 0 && (p as PageNum) < data.document_info.page_count {
                    //data.page_number = p as PageNum;
                    data.set_visible_scroll_position(ctx.window_id(), p as PageNum, None);
                    data.select_page(p as PageNum);
                }
            }
        }
    }

    // fn update(&mut self,
    //           child: &mut W,
    //           ctx: &mut UpdateCtx<'_, '_>,
    //           old_data: &PdfViewState,
    //           data: &PdfViewState,
    //           env: &Env) {

    //     let offset = data.goto_offset.parse::<i32>().unwrap_or(data.document_info.page_offset);
    //     let page = data.goto_page.parse::<i32>().unwrap_or(data.page_number as i32);

    //     let p = page + offset;
    //     if p >= 0 && p < data.document_info.page_count.try_into().unwrap() && p as usize != data.page_number {
    //         data.set_visible_scroll_position(ctx.window_id(), page, None);
    //     }
    // }
}

impl<W: Widget<PdfViewState>> Controller <PdfViewState, W> for FindController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut PdfViewState,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) => {
                if cmd.is(FOCUS_FIND_TEXTBOX) {
                    ctx.request_focus();
//                    ctx.set_handled();
                } else {
                    child.event(ctx, event, data, env);
                }
            }
            Event::KeyDown(e) => {
                if e.key == Key::Escape {
                    ctx.resign_focus();
                    data.search_results.borrow_mut().clear();
                    data.window_mode = WindowMode::Normal;
                    ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Normal));
                    ctx.set_handled();
                } else if e.mods.ctrl() && e.key == Key::Character("g".to_string()) {
                    ctx.resign_focus();
                    data.search_results.borrow_mut().clear();
                    data.window_mode = WindowMode::Goto;
                    ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Goto));
                    ctx.set_handled();
                } else {
                    child.event(ctx, event, data, env);
                }
            }
            _ => child.event(ctx, event, data, env),
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
            LifeCycle::WidgetAdded => {
                ctx.submit_command(FOCUS_FIND_TEXTBOX);
                child.lifecycle(ctx, event, data, env);
            }
            _ => child.lifecycle(ctx, event, data, env),
        }
    }

    fn update(
        &mut self,
        child: &mut W,
        ctx: &mut UpdateCtx<'_, '_>,
        old_data: &PdfViewState,
        data: &PdfViewState,
        env: &Env,
    ) {
        if data.find_goal != old_data.find_goal {
            ctx.submit_command(START_SEARCH); // handled by the scrollbar so it can request animation frames as it's redrawn to show search progress
        }
        child.update(ctx, old_data, data, env);
    }
}
