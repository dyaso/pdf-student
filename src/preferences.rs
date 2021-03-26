use druid::widget::prelude::*;
use druid::{
    commands as sys_cmds,
    Affine,
    AppDelegate,
    AppLauncher,
    ArcStr,
    Color,
    Command,
    DelegateCtx,
    FileInfo,
    FontDescriptor,
    FontStyle,
    FontWeight,
    Handled,
    Key,
    Lens,
    LensExt,
    LocalizedString,
    Modifiers,
    MouseButton,
    MouseEvent,
    PlatformError,
    Point,
    Rect,
    Selector,
    SysMods,
    Target,
    TextAlignment,
    TextLayout,
    UnitPoint,
    WidgetExt, //for https://docs.rs/druid/0.7.0/druid/widget/trait.Controller.html#a-textbox-that-takes-focus-on-launch
    WidgetPod,
    WindowDesc,
    WindowId,
};

use druid::widget::{
    Align, Axis, Container, Controller, Flex, Label, LineBreaking, Padding, Painter, RadioGroup,
    Scope, ScopeTransfer, Slider, Split, TextBox, ViewSwitcher,
};

use druid::commands::{COPY, CUT, PASTE, SHOW_PREFERENCES, UNDO};

use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::path::{Path, PathBuf};

use crate::AppState;

#[derive(Clone, Data, Debug, PartialEq, Serialize, Deserialize)]
pub enum DoubleClickAction {
    CropMode,
    SwitchScrollDirection,
}
impl Default for DoubleClickAction {
    fn default() -> Self {
        DoubleClickAction::CropMode
    }
}

#[derive(Clone, Copy, Data, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScrollbarLayout {
    Grid,
    Fractal,
}

impl Default for ScrollbarLayout {
    fn default() -> Self {
        ScrollbarLayout::Grid
    }
}

#[derive(Clone, Debug, Data, Default, Serialize, Deserialize, Lens)]
pub struct Preferences {
    pub doubleclick_action: DoubleClickAction,
    pub syncable_data_directory: String,
    pub brightness_inversion_amount: f64,
    pub scrollbar_layout: ScrollbarLayout,
}

impl Preferences {
    pub fn new() -> Self {
        let mut syncable_data_directory = "".to_string();

        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "PDF Student") {
            syncable_data_directory = PathBuf::from(proj_dirs.data_local_dir())
                .display()
                .to_string();
        }

        Preferences {
            doubleclick_action: DoubleClickAction::CropMode,
            syncable_data_directory,
            brightness_inversion_amount: 0.97,
            scrollbar_layout: ScrollbarLayout::Grid,
        }
    }
}

pub fn make_preferences_window() -> impl Widget<AppState> {
    Flex::column()
        .with_flex_child(
            Flex::row()
                .with_flex_child(
                    Align::new(UnitPoint::RIGHT,
                        Label::new(LocalizedString::new("Double-clicking on a PDF view window: "))
                            .with_line_break_mode(LineBreaking::WordWrap)
                            .with_text_alignment(TextAlignment::End)

                    )
                    , 1.)
                .with_flex_child(
                    Align::new(UnitPoint::LEFT,
                        RadioGroup::new(vec![
                            ("lets you edit page crop margins", DoubleClickAction::CropMode),
                            ("switches scroll direction", DoubleClickAction::SwitchScrollDirection),
                        ]).padding(5.0)
                        .lens(Preferences::doubleclick_action).lens(AppState::preferences)
                        )
                    , 1.)
            , 1.)
        .with_flex_child(
            Flex::row()
                .with_flex_child(
                    Align::new(UnitPoint::RIGHT,
                        Label::new(LocalizedString::new("Place to store sync-able meta info about PDFs, maybe somewhere in your Dropbox folder: \r \rPage tags, bookmarks, reading positions, and custom colour inversion selections will follow you between machines. \r \rThese data are stored as lots of tiny files, so best to put them in their own folder."))
                            .with_line_break_mode(LineBreaking::WordWrap)
                            //.with_text_alignment(TextAlignment::End)
                            .padding(5.0)
                    )
                    , 1.)
                .with_flex_child(
                    Align::new(UnitPoint::LEFT,
                        TextBox::multiline()
        .expand()
        .padding(3.0)
        //.controller(TextCopyPasteController)

                            .lens(Preferences::syncable_data_directory).lens(AppState::preferences))
                , 1.)
            , 2.)
        .with_flex_child(
            Flex::row()
                .with_flex_child(
                    Align::new(UnitPoint::RIGHT,
                        Label::new(LocalizedString::new("Brightness inversion: "))
                        )
                        .expand()
                        .padding(5.0)
                    ,1.)
                .with_flex_child(
                    Align::new(UnitPoint::LEFT,
                        Flex::row()
                            .with_child(Label::new(LocalizedString::new("None")))
                            .with_child(
                                Flex::column()
                                    .with_child(Slider::new()
                                    .lens(Preferences::brightness_inversion_amount).lens(AppState::preferences))

                                    .with_spacer(4.0)
                                    .with_child(Label::new(|data: &AppState, _: &_| {
                                        format!("{:3.0}%", data.preferences.brightness_inversion_amount * 100.0)
                                    }))
                                )
                            .with_child(Label::new(LocalizedString::new("Total")))
                            )
                    ,1.)
            ,1.)

        .with_flex_child(
            Flex::row()
                .with_flex_child(
                    Align::new(UnitPoint::RIGHT,
                        Label::new(LocalizedString::new("Default page overview layout: "))
                            .with_line_break_mode(LineBreaking::WordWrap)
                            .with_text_alignment(TextAlignment::End)

                    )
                    , 1.)
                .with_flex_child(
                    Align::new(UnitPoint::LEFT,
                        RadioGroup::new(vec![
                            ("grid -- misleading spacing", ScrollbarLayout::Grid),
                            ("fractal -- unpredictable shape, smaller", ScrollbarLayout::Fractal),
                        ]).padding(5.0)
                        .lens(Preferences::scrollbar_layout).lens(AppState::preferences)
                        )
                    , 1.)
            , 1.)
        .padding(2.).controller(TextCopyPasteController)
}

#[derive(Debug, Default)]
pub struct TextCopyPasteController;

impl<W: Widget<AppState>> Controller<AppState, W> for TextCopyPasteController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        match event {
            Event::KeyDown(key_event) => {
                if
                //key_event.state == druid::Code::Down
                //&&
                key_event.code == druid::Code::KeyX
                    && key_event.mods & Modifiers::CONTROL == Modifiers::CONTROL
                {
                    ctx.submit_command(CUT);
                } else if
                // key_event.state == druid::Code::Down
                //&&
                key_event.code == druid::Code::KeyC
                    && key_event.mods & Modifiers::CONTROL == Modifiers::CONTROL
                {
                    ctx.submit_command(COPY);
                } else if
                //key_event.state == druid::Code::Down
                //&&
                key_event.code == druid::Code::KeyV
                    && key_event.mods & Modifiers::CONTROL == Modifiers::CONTROL
                {
                    ctx.submit_command(PASTE);
                // todo : make undo work
                // also double click -> word select, triple click -> line select, ctrl-held cursor movement
                } else if key_event.code == druid::Code::KeyZ
                    && key_event.mods & Modifiers::CONTROL == Modifiers::CONTROL
                {
                    ctx.submit_command(UNDO);
                } else {
                    child.event(ctx, event, data, env);
                }
            }
            other => child.event(ctx, other, data, env),
        }
    }

    fn update(
        &mut self,
        child: &mut W,
        ctx: &mut UpdateCtx,
        old_data: &AppState,
        data: &AppState,
        env: &Env,
    ) {
        if data.preferences.doubleclick_action != old_data.preferences.doubleclick_action {
            //         env.set(DOUBLECLICK_ACTION, doubleclickaction_to_u64(&data.preferences.doubleclick_action));
        }
        child.update(ctx, old_data, data, env);
    }
}
