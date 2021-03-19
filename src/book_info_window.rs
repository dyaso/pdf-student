use druid::kurbo::BezPath;
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    commands as sys_cmds,
    lens,
    Affine,
    AppDelegate,
    AppLauncher,
    ArcStr,
    Color,
    Command,
    ContextMenu,
    DelegateCtx,
    FileDialogOptions,
    FileInfo,
    FileSpec,
    FontDescriptor,
    FontStyle,
    FontWeight,
    Handled,
    Key,
    Lens,
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

use druid::im::{HashMap, Vector};

use druid::lens::{Identity, LensExt};

use druid::widget::{
    Align, Axis, Button, Container, Controller, Either, Flex, Label, LineBreaking, List, Padding,
    Painter, RadioGroup, Scope, ScopeTransfer, Scroll, SizedBox, Slider, Split, TextBox,
    ViewSwitcher,
};

use std::convert::TryInto;
use std::path::{Path, PathBuf};

use crate::OPEN_BOOK_WITH_FINGERPRINT;
use crate::SAVE_DOCUMENT_INFO;
use crate::{AppState, Document, DocumentInfo, Fingerprint, RecentDocumentsWithLocations};

#[derive(Clone, Data, Lens)]
struct EditableInfoCard {
    info: DocumentInfo,
    being_edited: bool,
    selected: bool,
    // finished_editing: bool,
}

// Info card that remembers if it's currently being edited
struct EditableInfoCardTransfer;

impl ScopeTransfer for EditableInfoCardTransfer {
    type In = ((Fingerprint, Vector<DocumentInfo>), DocumentInfo);

    type State = EditableInfoCard;

    fn read_input(&self, my_state: &mut Self::State, ((selected_fp, _), info): &Self::In) {
        my_state.info = info.clone();
        my_state.selected = my_state.info.fingerprint == *selected_fp;
    }

    fn write_back_input(&self, my_state: &Self::State, external: &mut Self::In) {
        external.1 = my_state.info.clone();
    }
}

// List with a textbox which can be used to filter the list
#[derive(Clone, Data, Lens)]
struct FilterableList {
    search_filter: String,
    matches: Vector<DocumentInfo>,
    selected_result: Fingerprint,
    all_books: HashMap<Fingerprint, DocumentInfo>,
    access_history: Vector<Fingerprint>,
}

struct FilterableListTransfer;

impl ScopeTransfer for FilterableListTransfer {
    type In = AppState;

    type State = FilterableList;

    fn read_input(&self, my_state: &mut Self::State, external: &Self::In) {
        my_state.all_books = external.all_local_documents_info.clone();
        my_state.access_history = external.recent_document_locations.fingerprints.clone();
    }

    fn write_back_input(&self, my_state: &Self::State, external: &mut Self::In) {
        external.all_local_documents_info = my_state.all_books.clone();
        //external.access_history: recent_document_locations.fingerprints.clone(),
    }
}

const BOOK_CARD_HEIGHT: f64 = 80.;
const LIST_WIDGET_ID: WidgetId = WidgetId::reserved(1);

pub fn make_book_info_window(_state: &AppState, _doc_idx: usize) -> WindowDesc<AppState> {
    let ui = Scope::from_function(
        |app_state| FilterableList {
            search_filter: String::new(),
            matches: Vector::<DocumentInfo>::new(),
            selected_result: Fingerprint::new(),
            all_books: app_state.all_local_documents_info.clone(),
            access_history: app_state.recent_document_locations.fingerprints,
        },
        FilterableListTransfer,
        Flex::column()
            .with_child(Padding::new(
                (0., 15., 0., 15.),
                Flex::row()
                    .with_default_spacer()
                    .with_child(
                        Flex::row()
                            .with_child(Label::new("Filter list of recent books: "))
                            .with_child(
                                SizedBox::new(
                                    TextBox::new()
                                        .with_placeholder("word fragments go here")
                                        .lens(FilterableList::search_filter)
                                        .controller(BookListController),
                                )
                                // .expand()
                                .width(200.),
                            ),
                    )
                    .with_default_spacer()
                    .with_child(Align::new(UnitPoint::CENTER, Label::new(" or ")))
                    .with_default_spacer()
                    .with_child(Button::new("Open something new").on_click(
                        |ctx, _data: &mut FilterableList, _env| {
                            let pdf = FileSpec::new("PDF file", &["pdf"]);
                            let open_dialogue_options = FileDialogOptions::new()
                                .allowed_types(vec![pdf])
                                .default_type(pdf)
                                //.default_name(default_save_name)
                                //.name_label("Target")
                                .title("Choose a PDF file to open")
                                .button_text("Open");
                            ctx.submit_command(Command::new(
                                druid::commands::SHOW_OPEN_PANEL,
                                open_dialogue_options,
                                Target::Auto,
                            ));
                        },
                    ))
                    .with_default_spacer(),
            ))
            .with_flex_child(
                Scroll::new(
                    List::new(|| {
                        Scope::from_function(
                            |((_, _), doc_info)| EditableInfoCard {
                                selected: false,
                                info: doc_info,
                                being_edited: false,
                            },
                            EditableInfoCardTransfer,
                            Either::new(
                                |card: &EditableInfoCard, _e: &Env| card.being_edited,
                                SizedBox::new(
                                    Flex::row()
                                        // .height(50.0)
                                        .with_flex_child(
                                            TextBox::multiline().expand().lens(
                                                //lens!((DocumentInfo, bool),0)
                                                EditableInfoCard::info
                                                    .then(DocumentInfo::description),
                                            ), //Identity.map(|um : &(DocumentInfo, bool)| um.0.description.clone(),
                                            //|um: &mut (DocumentInfo, bool), s| um.0.description = s.clone()))
                                            1.,
                                        )
                                        .with_child(Button::new("Finish editing").on_click(
                                            |ctx, data: &mut EditableInfoCard, _env| {
                                                data.being_edited = false;
                                                ctx.submit_command(
                                                    SAVE_DOCUMENT_INFO
                                                        .with(data.info.fingerprint.clone()),
                                                )
                                            },
                                        )),
                                )
                                .expand()
                                .height(BOOK_CARD_HEIGHT),
                                Flex::row()
                                    .with_flex_child(
                                        Label::new(|item: &EditableInfoCard, _env: &_| {
                                            format!(
                                                "[{}/{}] {}",
                                                item.info.most_recent_page,
                                                item.info.page_count,
                                                item.info.description
                                            )
                                        })
                                        .with_line_break_mode(LineBreaking::WordWrap)
                                        // .background(druid::theme::PRIMARY_DARK)
                                        .align_vertical(UnitPoint::LEFT)
                                        .padding(10.0)
                                        .expand()
                                        .height(80.0)
                                        .on_click(|ctx, card: &mut EditableInfoCard, _env| {
                                            ctx.submit_command(
                                                OPEN_BOOK_WITH_FINGERPRINT
                                                    .with(card.info.fingerprint.clone()),
                                            );
                                        })
                                        .background(
                                            Painter::new(|ctx, data: &EditableInfoCard, _env| {
                                                let bounds = ctx.size().to_rect();
                                                if data.selected {
                                                    ctx.fill(
                                                        bounds,
                                                        &Color::rgba8(200, 20, 200, 150),
                                                    ); //&env.get(druid::theme::BACKGROUND_LIGHT));
                                                }
                                            }),
                                        ),
                                        1.,
                                    )
                                    .with_child(Button::new("Edit description").on_click(
                                        |_ctx, data: &mut EditableInfoCard, _env| {
                                            data.being_edited = true
                                        },
                                    )),
                            ),
                        ) // Scope for info cards
                    }) // List of cards
                    .lens(lens::Identity.map(
                        // Expose shared data with children data
                        |d: &FilterableList| {
                            (
                                (d.selected_result.clone(), d.matches.clone()),
                                d.matches.clone(),
                            )
                        },
                        move |d: &mut FilterableList,
                              ((_selected_fingerprint, original_list), modified_list): (
                            (Fingerprint, Vector<DocumentInfo>),
                            Vector<DocumentInfo>,
                        )| {
                            d.find_matches();

                            // todo: let the user edit the filename
                            for (i, info) in modified_list.iter().enumerate() {
                                if original_list[i].description != info.description {
                                    if let Some(doc_info) = d.all_books.get_mut(&info.fingerprint) {
                                        doc_info.description = info.description.clone();
                                    }
                                }
                            }
                        },
                    )),
                )
                .vertical()
                .controller(ScrollController)
                .with_id(LIST_WIDGET_ID),
                1.,
            ),
    );

    WindowDesc::new(ui).title(LocalizedString::new("Open another book"))
}

struct BookListController;

impl FilterableList {
    fn find_matches(&mut self) {
        let search = self.search_filter.to_lowercase();

        self.matches.clear();

        for fp in self.access_history.iter().rev() {
            if let Some(info) = &self.all_books.get(fp) {
                if fuzzy_match(&info.description.to_lowercase(), &search) {
                    self.matches.push_back((*info).clone());
                }
            }
        }
    }

    fn select_item(&mut self, ctx: &mut EventCtx, idx: usize) {
        if let Some(item) = self.matches.get(idx) {
            self.selected_result = item.fingerprint.clone();
    
            ctx.submit_command(
                REPOSITION
                    .with(Rect::from_origin_size(
                        (0., BOOK_CARD_HEIGHT * idx as f64),
                        (10., BOOK_CARD_HEIGHT),
                    ))
                    .to(LIST_WIDGET_ID),
            );
        }

    }
}

const REPOSITION: Selector<Rect> = Selector::new("reposition-scroll");

struct ScrollController;

//impl Controller<Scroll<FilterableList, List>> for ScrollController {

//impl<T> Controller<Scroll<FilterableList, List<FilterableList>>> for ScrollController {

impl<W: Widget<FilterableList>> Controller<FilterableList, Scroll<FilterableList, W>>
    for ScrollController
{
    //impl<W: Widget<FilterableList>> Controller<FilterableList, W> for ScrollController {
    fn event(
        &mut self,
        child: &mut Scroll<FilterableList, W>,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut FilterableList,
        env: &Env,
    ) {
        if let Event::Command(cmd) = event {
            if let Some(rect) = cmd.get(REPOSITION) {
                child.scroll_to(*rect);
            } else {
                child.event(ctx, event, data, env);
            }
        } else {
            child.event(ctx, event, data, env);
        }
    }
}

impl<W: Widget<FilterableList>> Controller<FilterableList, W> for BookListController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut FilterableList,
        env: &Env,
    ) {
        match event {
            Event::WindowConnected => {
                ctx.request_focus();
                data.find_matches();
                child.event(ctx, event, data, env)
            }

            Event::KeyDown(e) => {
                println!("{}", e.key);
                match e.key {
                    druid::keyboard_types::Key::ArrowDown => {
                        let mut idx = 0;
                        if let Some(i) = data
                            .matches
                            .iter()
                            .position(|x| x.fingerprint == data.selected_result)
                        {
                            idx = usize::min(i + 1, data.matches.len() - 1);
                        }

                        data.select_item(ctx, idx);
                    }
                    druid::keyboard_types::Key::ArrowUp => {
                        let mut idx = data.matches.len() - 1;
                        if let Some(i) = data
                            .matches
                            .iter()
                            .position(|x| x.fingerprint == data.selected_result)
                        {
                            idx = i.saturating_sub(1);
                        }
                        data.select_item(ctx, idx);
                    }
                    druid::keyboard_types::Key::Enter => {
                        ctx.submit_command(
                            OPEN_BOOK_WITH_FINGERPRINT.with(data.selected_result.clone()),
                        );
                    }
                    _ => {
                        //                        println!("{}",e.key);
                        child.event(ctx, event, data, env);

                        data.find_matches();
                    }
                }
            }
            _ => {
                //println!("{:?}",event);
                child.event(ctx, event, data, env);
                data.find_matches();
            }
        }
    }

    fn lifecycle(
        &mut self,
        child: &mut W,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &FilterableList,
        env: &Env,
    ) {
        match event {
            LifeCycle::HotChanged(_now) => child.lifecycle(ctx, event, data, env),
            _ => child.lifecycle(ctx, event, data, env),
        }
    }
}

// `candidate` must contain all the characters of `filter`, in the same order, not necessarily next to each other
fn fuzzy_match_word(candidate: &str, filter: &str) -> bool {
    let mut q = filter.chars();
    let mut c = candidate.chars();
    let mut qh: Option<char>;
    let mut ch: Option<char>;
    loop {
        qh = q.next();
        if qh == None {
            return true;
        }
        loop {
            ch = c.next();
            if ch == None {
                return false;
            }
            if ch == qh {
                break;
            }
        }
    }
}

fn fuzzy_match(candidate: &str, filter: &str) -> bool {
    let words = &mut candidate.split_whitespace(); //.split("-").split("_").split(".").split("/").split("\\");

    for filter_word in filter.split_whitespace() {
        if words
            .clone()
            .any(|w| fuzzy_match_word(&w.to_string(), &filter_word.to_string()))
        {
            continue;
        } else {
            return false;
        }
    }
    true
}
