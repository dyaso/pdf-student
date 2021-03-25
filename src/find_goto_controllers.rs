use druid::widget::prelude::*;
use druid::{
    Affine, AppLauncher, Color, Command, FileDialogOptions, FileSpec, FontDescriptor,
    FontStyle, FontWeight, Handled, Lens, LocalizedString, Menu, MenuItem, MouseButton,
    MouseEvent, Point, Rect, Selector, SysMods, Target, TextLayout, Vec2, WindowDesc, WindowId,
};

use druid::widget::{ControllerHost,
    Align, Axis, Container, Controller, Flex, Label, LineBreaking, Padding, Painter, RadioGroup,
    Scope, ScopeTransfer, Slider, Split, TextBox, ViewSwitcher, WidgetExt,
};

use druid::keyboard_types::Key;


use crate::pdf_view::{PdfViewState, WindowMode, SET_WINDOW_MODE};

struct FindController;

pub fn make_find_ui() -> impl Widget<PdfViewState> {
	Flex::row()
	.with_child(Label::new("Find exact phrase, case insensitively: ").controller(FindController))
	.with_child(TextBox::new().lens(PdfViewState::find_goal).controller(FindController))
}

impl<W: Widget<PdfViewState>> Controller<PdfViewState, W> for FindController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut PdfViewState,
        env: &Env,
    ) {
    	match event {
	        Event::KeyDown(e) => {
				if e.key == Key::Enter {
					ctx.resign_focus();
                    ctx.submit_command(SET_WINDOW_MODE.with(WindowMode::Normal));
                } else {
		    		 child.event(ctx, event, data, env);
                }
	        },
	    	_ => child.event(ctx, event, data, env),
    	}
	}

}
