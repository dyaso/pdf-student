#![allow(unused_imports)]
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

use druid::kurbo::BezPath;
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, Text, TextLayoutBuilder};
use druid::widget::prelude::*;
use druid::{
    commands as sys_cmds,
    Affine,
    AppDelegate,
    AppLauncher,
    ArcStr,
    Color,
    Command,
    ContextMenu,
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

use std::convert::TryInto;

use druid::commands::{COPY, CUT, PASTE, SHOW_PREFERENCES, UNDO};

use mupdf::pdf::PdfDocument;
use mupdf::{Colorspace, Matrix, Page};

//use path_slash::PathExt;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use druid::im::{HashMap, HashSet, Vector};

use druid::lens;

use std::fs;

mod pdf_view;
use crate::pdf_view::make_pdf_view_window;
use crate::pdf_view::PdfViewState;
use crate::pdf_view::NEW_VIEW_WITH_PARENT;

mod pdf_text_widget;
use crate::pdf_text_widget::PdfTextWidget;

mod scrollbar_widget;
use scrollbar_widget::ScrollbarWidget;

mod book_info_window;

mod preferences;
use preferences::{make_preferences_window, Preferences};

type PageNum = usize; // mupdf-rs uses i32, i'm not sure why it's signed

const UNIT_SQUARE: Rect = Rect {
    x0: 0.,
    x1: 1.,
    y0: 0.,
    y1: 1.,
};

#[derive(Debug, Clone, Data, Serialize, Deserialize, PartialEq)]
enum CropMargins {
    AllPagesSame(Rect),
    DistinguishEvenAndOddPages(Rect, Rect),
}

// fingerprinting PDFs https://www.seanh.cc/2017/11/22/pdf-fingerprinting/
fn fingerprint(document: &PdfDocument) -> Option<String> {
    let trailer = document.trailer().unwrap();
    let pdf_id = trailer.get_dict("ID").expect("unwrap 1 failed");

    match pdf_id {
        Some(pdf_id) => {
            let fingerprint = pdf_id
                .resolve()
                .unwrap()
                .unwrap()
                .get_array(0)
                .unwrap()
                .unwrap()
                .to_string();
            Some(fingerprint[1..fingerprint.len() - 1].to_string())
        }
        None => None,
    }
}

use crate::pdf_text_widget::lerp_rect;

type Fingerprint = String;

pub const OPEN_BOOK_WITH_FINGERPRINT: Selector<Fingerprint> =
    Selector::new("open-book-with-fingerprint");

#[derive(Clone, Debug, Data, Serialize, Deserialize, PartialEq, Lens)]
pub struct DocumentInfo {
    default_margins: CropMargins,
    custom_margins: HashMap<PageNum, Rect>,

    color_inversion_rectangles: HashMap<PageNum, Vector<Rect>>,

    fingerprint: Fingerprint,

    contents_page: PageNum,
    bookmarks: HashMap<String, PageNum>,
    tags: HashMap<PageNum, u16>,
    #[serde(default)] // https://serde.rs/field-attrs.html
    most_recent_page: PageNum,
    #[serde(default)]
    page_count: PageNum,
    // #[serde(default)]
    // prerequistes: Vector<Fingerprint>,
    // #[serde(default)]
    // requisite_for: Vector<Fingerprint>,
    #[serde(default)]
    description: String,
}

use CropMargins::{AllPagesSame, DistinguishEvenAndOddPages};

impl DocumentInfo {
    pub fn from_fingerprint(data_dir: &PathBuf, fingerprint: &str) -> Self {
        //        if let Some(dir) = data_dir {
        let mut path: PathBuf = data_dir.clone();
        path.push(&fingerprint);
        path.set_extension("json");

        // let mtime = match &path.metadata() {
        //                 Ok(metadata) => metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        //                 Err(_) => SystemTime::UNIX_EPOCH};

        match fs::read_to_string(&path) {
            Ok(serialized) => match serde_json::from_str(&serialized) {
                Ok(loaded_info) => return loaded_info,
                Err(e) => println!("error loading doc info: {}", e),
            },
            Err(e) => println!("doc info loading error: {} {:?}", e, path),
        }

        DocumentInfo {
            default_margins: AllPagesSame(Rect::new(0.05, 0.05, 0.95, 0.95)),
            custom_margins: HashMap::<PageNum, Rect>::new(),

            fingerprint: fingerprint.to_string(),

            color_inversion_rectangles: HashMap::<PageNum, Vector<Rect>>::new(),
            bookmarks: HashMap::<String, usize>::new(),

            tags: HashMap::<PageNum, u16>::new(),
            contents_page: 0,
            most_recent_page: 0,
            page_count: 0,

            description: String::new(),
            // prerequistes: Vector::<Fingerprint>::new(),
            // requisite_for: Vector::<Fingerprint>::new(),
        }
    }

    fn has_custom_margins(&self, page_number: PageNum) -> bool {
        self.custom_margins.contains_key(&page_number)
    }
    fn are_all_pages_same(&self) -> bool {
        matches!(self.default_margins, AllPagesSame(_))
    }
    fn are_even_and_odd_distinguished(&self) -> bool {
        match self.default_margins {
            AllPagesSame(_) => false,
            DistinguishEvenAndOddPages(_, _) => true,
        }
    }
    fn toggle_custom_margins(&mut self, page_number: PageNum) {
        if self.has_custom_margins(page_number) {
            self.custom_margins.remove(&page_number);
        } else {
            self.custom_margins.insert(
                page_number,
                match self.default_margins {
                    AllPagesSame(margins) => margins,
                    DistinguishEvenAndOddPages(even, odd) => {
                        if page_number % 2 == 0 {
                            even
                        } else {
                            odd
                        }
                    }
                },
            );
        }
    }

    fn toggle_even_odd_page_distinction(&mut self, page_number: PageNum) {
        self.default_margins = match self.default_margins {
            AllPagesSame(margins) => DistinguishEvenAndOddPages(margins, margins),
            DistinguishEvenAndOddPages(even, odd) => {
                if page_number % 2 == 0 {
                    AllPagesSame(even)
                } else {
                    AllPagesSame(odd)
                }
            }
        }
    }

    fn page_margins_in_normalized_coords(&self, page_number: PageNum) -> Rect {
        match self.custom_margins.get(&page_number) {
            Some(margins) => *margins,
            None => match self.default_margins {
                AllPagesSame(margins) => margins,
                DistinguishEvenAndOddPages(even, odd) => {
                    if page_number % 2 == 0 {
                        even
                    } else {
                        odd
                    }
                }
            },
        }
    }

    fn set_page_margins(&mut self, page_number: PageNum, rect: Rect) {
        if self.custom_margins.contains_key(&page_number) {
            self.custom_margins.insert(page_number, rect);
        } else {
            match self.default_margins {
                AllPagesSame(_) => self.default_margins = AllPagesSame(rect),
                DistinguishEvenAndOddPages(even, odd) => {
                    if page_number % 2 == 0 {
                        self.default_margins = DistinguishEvenAndOddPages(rect, odd);
                    } else {
                        self.default_margins = DistinguishEvenAndOddPages(even, rect);
                    }
                }
            }
        }
    }

    fn weighted_page_margins_in_normalized_coords(
        &self,
        page_number: PageNum,
        crop_weight: f64,
    ) -> Rect {
        lerp_rect(
            &UNIT_SQUARE,
            &self.page_margins_in_normalized_coords(page_number),
            crop_weight,
        )
    }

    fn tag_bits(&self, page: PageNum) -> u16 {
        *self.tags.get(&page).unwrap_or(&0)
    }

    fn clear_tags(&mut self, page: PageNum) {
        self.tags.remove(&page);
    }

    fn set_tags(&mut self, page: PageNum, tags: u16) {
        self.tags.insert(page, tags);
    }

    fn toggle_tag_bit(&mut self, page: PageNum, bit: u32) {
        let t = self.tag_bits(page);
        self.tags.insert(page, t ^ (1 << bit));
    }

    pub fn add_bookmark(&mut self, c: &str, page: usize) {
        self.bookmarks.insert(c.to_string(), page);
    }

    pub fn lookup_bookmark(&self, c: String) -> Option<&usize> {
        self.bookmarks.get(&c)
    }
}

#[derive(Clone, Debug, Data)]
pub struct Hyperlink {
    bounds: Rect,
    link: (usize, String),
}

#[derive(Clone, Debug, Data)]
pub struct Document {
    //    info: DocumentInfo,
    fingerprint: Fingerprint,
    pdf_file: Arc<PdfDocument>,
    filepath: String,
    user_facing_path: String,
    current_page_number_in_window_id: HashMap<WindowId, PageNum>,
    reverse_bookmarks: HashMap<usize, String>,
    hyperlinks: HashMap<usize, Option<Vector<Hyperlink>>>,
    rcurrent_page_number_in_window_id: Arc<RefCell<HashMap<WindowId, PageNum>>>,

    doc_info_changed: bool,
}

impl Document {
    pub fn from_pdf_and_info(
        pdf_doc: PdfDocument,
        //fingerprint: Fingerprint,
        info: &DocumentInfo,
        filepath: String,
        user_facing_path: String,
        rcurrent_page: &Arc<RefCell<HashMap<WindowId, PageNum>>>,
    ) -> Self {
        let mut doc = Document {
            fingerprint: info.fingerprint.clone(),
            pdf_file: Arc::new(pdf_doc),
            filepath,
            user_facing_path,
            current_page_number_in_window_id: HashMap::<WindowId, PageNum>::new(),
            reverse_bookmarks: HashMap::<usize, String>::new(),
            hyperlinks: HashMap::<usize, Option<Vector<Hyperlink>>>::new(),
            rcurrent_page_number_in_window_id: (*rcurrent_page).clone(),
            doc_info_changed: false,
        };
        doc.generate_reverse_bookmarks(&info);
        doc
    }

    pub fn load_page(&self, page_number: PageNum) -> Page {
        self.pdf_file
            .load_page(page_number as i32)
            .expect("Unable to load page")
    }

    pub fn get_page_size_in_points(&self, page_number: PageNum) -> Size {
        let page = self.load_page(page_number);
        let bounds = page.bounds().expect("Unable to get page bounds");
        Size {
            width: bounds.width() as f64,
            height: bounds.height() as f64,
        }
    }

    pub fn generate_reverse_bookmarks(&mut self, info: &DocumentInfo) {
        self.reverse_bookmarks.clear();

        for (chr, page) in &info.bookmarks {
            self.reverse_bookmarks.insert(*page, (*chr).to_string());
        }
    }

    pub fn check_for_bookmark(&self, page: usize) -> Option<&String> {
        self.reverse_bookmarks.get(&page)
    }
}

use std::path::{Path, PathBuf};

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

#[derive(Debug, Clone, Default, Data, Serialize, Deserialize, PartialEq)]
pub struct RecentDocumentsWithLocations {
    pub fingerprints: Vector<Fingerprint>,
    pub locations: HashMap<Fingerprint, Vector<String>>,
}

// impl RecentDocumentsWithLocations {
//     fn new() -> Self {
//         Self { fingerprints: Vector::<Fingerprint>::new(),
//             locations: HashMap::<Fingerprint, Vector<String>>::new()}
//     }
// }

fn put_thing_at_back_of_set_vector<T: Clone + std::cmp::PartialEq>(
    set_vec: &mut Vector<T>,
    thing: T,
) {
    if set_vec.len() == 1 && set_vec[0] == thing {
        return;
    }

    let mut new_vec = Vector::<T>::new();

    for l in set_vec.iter() {
        if *l != thing {
            new_vec.push_back(l.clone());
        }
    }

    new_vec.push_back(thing);

    *set_vec = new_vec
}

use std::time::SystemTime;

#[derive(Clone, Default, Data, Lens)]
pub struct AppState {
    loaded_documents: Vector<Document>,
    all_local_documents_info: HashMap<Fingerprint, DocumentInfo>,
    recent_document_locations: RecentDocumentsWithLocations,
    local_data_directory: Option<Arc<PathBuf>>, // contains "<fingerprint>.local.json" files containing the most recently seen filepaths of books on this machine
    syncable_data_directory: Option<Arc<PathBuf>>, // contains "<fingerprint>.json" files containing book info (page progress, crop margins, eventually highlighted facts of interest) we want sync'd between machines
    preferences: Preferences,
    rcurrent_page_number_in_window_id: Arc<RefCell<HashMap<WindowId, PageNum>>>,

    filesystem_watcher: Option<Arc<RecommendedWatcher>>,
    just_saved: HashSet<String>, // after saving doc info, ignore the next notification from the filesystem watcher about the file changing

    search_filter: String,
}

impl AppState {
    pub fn new() -> Self {
        let mut local_data_directory = None;
        let mut syncable_data_directory = None;
        let mut preferences = Preferences::new();

        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "PDF Student") {
            local_data_directory = Some(Arc::new(PathBuf::from(proj_dirs.data_local_dir())));

            let mut prefs_path: PathBuf = PathBuf::from(proj_dirs.data_local_dir());
            prefs_path.push("preferences");
            prefs_path.set_extension("json");
            match fs::read_to_string(prefs_path) {
                Ok(serialized) => {
                    if let Ok(prefs) = serde_json::from_str(&serialized) {
                        preferences = prefs;
                    }
                }
                Err(e) => println!("unable to read stored preferences: {}", e),
            }

            syncable_data_directory = Some(Arc::new(PathBuf::from(proj_dirs.data_local_dir())));
        }

        let mut doc_locs: Option<RecentDocumentsWithLocations> = None;

        if let Some(dir) = local_data_directory.clone() {
            let mut path = PathBuf::from(&*dir);
            path.push("file_locations");
            path.set_extension("json");

            if let Ok(serialized) = fs::read_to_string(path) {
                let file_locations = serde_json::from_str(&serialized);
                match file_locations {
                    Ok(file_locations) => {
                        doc_locs = Some(file_locations);
                    }
                    Err(e) => {
                        println!(
                            "Error reading list of last-seen-locations of documents: {}",
                            e
                        );
                    }
                }
            }
        }

        let mut all_local_documents_info = HashMap::<Fingerprint, DocumentInfo>::new();

        if let Some(documents_on_this_machine) = &doc_locs {
            let dir = preferences.syncable_data_directory.clone();
            let p = PathBuf::from(dir);
            for fp in documents_on_this_machine.fingerprints.iter() {
                all_local_documents_info
                    .insert(fp.clone(), DocumentInfo::from_fingerprint(&p, &fp));
            }
        }

        Self {
            loaded_documents: Vector::<Document>::new(),
            local_data_directory, //:    Arc::new(PathBuf::from(local_data_dir)),
            syncable_data_directory, //: Arc::new(PathBuf::from( sync_data_dir)),
            recent_document_locations: doc_locs
                .unwrap_or_else(RecentDocumentsWithLocations::default),
            rcurrent_page_number_in_window_id: Arc::<RefCell<HashMap<WindowId, PageNum>>>::new(
                RefCell::new(HashMap::<WindowId, PageNum>::new()),
            ),
            all_local_documents_info,
            filesystem_watcher: None,
            just_saved: HashSet::<String>::new(),
            preferences,
            ..AppState::default()
        }
    }

    fn already_loaded(&self, fingerprint: String) -> Option<usize> {
        for (i, doc) in self.loaded_documents.iter().enumerate() {
            if doc.fingerprint == fingerprint {
                return Some(i);
            }
        }
        None
    }

    // returns index of the file buffer if loading was successful
    pub fn load_file(&mut self, path: &Path) -> Option<usize> {
        if !path.exists() {
            return None;
        }
        let path_string: String = path
            .canonicalize()
            .expect("unable to canonicalize file path")
            .to_str()
            .expect("filepath could not be converted to string")
            .to_string();
        let mut user_facing_path = path_string.clone();
        if let Some(user_dirs) = directories::UserDirs::new() {
            let home = user_dirs.home_dir().to_str().unwrap();
            if path_string.starts_with(home) {
                user_facing_path = "~".to_string();
                let mut first = true;
                for s in path_string.split(home).skip(1) {
                    if !first {
                        user_facing_path.push_str(home);
                    }
                    user_facing_path.push_str(s);
                    first = false;
                }
            }
        }

        match mupdf::pdf::PdfDocument::open(&path_string) {
            Ok(pdf_doc) => {
                let fingerprint = fingerprint(&pdf_doc).unwrap_or_else(|| {
                    let mut file = File::open(&path_string).unwrap();

                    let mut buf = [0; 1024];

                    if let Err(e) = file.read_exact(&mut buf) {
                        println!("error reading PDF file to compute document fingerprint: {}",e);
                    }
                    let digest = md5::compute(&mut buf);

                    let res = format!("{:x}", digest);
                    res
                });

                let mut doc_info = DocumentInfo::from_fingerprint(
                    &PathBuf::from(&self.preferences.syncable_data_directory),
                    &fingerprint,
                );

                let mut changed = false;

                if doc_info.page_count == 0 {
                    // this is the first time we've seen this PDF
                    doc_info.page_count = pdf_doc.page_count().unwrap_or(0) as PageNum;
                    changed = true;
                }

                if doc_info.description.is_empty() {
                    // this is the first time we've seen this PDF
                    changed = true;
                    let title = pdf_doc.metadata(mupdf::document::MetadataName::Title);
                    let authors = pdf_doc.metadata(mupdf::document::MetadataName::Author);

                    // let trailer = pdf_doc.trailer().unwrap();
                    //                    let pdf_id = trailer.get_dict("ID").expect("unwrap 1 failed");
                    if let Ok(res) = title {
                        doc_info.description.push_str(&res);
                    }

                    if ! doc_info.description.is_empty() {
                        doc_info.description.push_str(user_facing_path.as_str());
                    }

                    if let Ok(res) = authors {
                        if ! res.is_empty() {
                            doc_info.description.push_str(" by ");
                            doc_info.description.push_str(&res);
                        }
                    }
                }

                self.all_local_documents_info
                    .insert(fingerprint.clone(), doc_info.clone());

                if changed {
                    self.save_document_info(&fingerprint);
                }

                // keep track of wherever we've seen this book before, so it can be opened from within the app's "Recently Opened" list
                // need to remember every place we've seen it, in case we see like a duplicate copy on removable media or something, don't want to forget the on-disk location
                let locations = self
                    .recent_document_locations
                    .locations
                    .entry(fingerprint.clone())
                    .or_insert_with(Vector::<String>::new);

                put_thing_at_back_of_set_vector(locations, path_string.clone());
                put_thing_at_back_of_set_vector(
                    &mut self.recent_document_locations.fingerprints,
                    fingerprint.clone(),
                );

                match self.already_loaded(fingerprint) {
                    None => {
                        self.loaded_documents.push_back(Document::from_pdf_and_info(
                            pdf_doc,
                            &doc_info,
                            path_string,
                            user_facing_path,
                            &self.rcurrent_page_number_in_window_id,
                        ));
                        let new_id = self.loaded_documents.len() - 1;
                        Some(new_id)
                    }
                    Some(id) => Some(id),
                }
            }
            Err(e) => {
                println!("Error opening PDF file: {}", e);
                None
            }
        }
    }

    pub fn open_book_with_fingerprint(&mut self, fp: &str) -> Option<usize> {
        // check if it's already loaded in documents
        // work through list of known locations, most recent first, trying to load file
        if let Some(idx) = self
            .loaded_documents
            .iter()
            .position(|doc| doc.fingerprint == *fp)
        {
            return Some(idx);
        }

        // for (i, doc) in &self.loaded_documents.iter().enumerate() {
        //     if doc.info == fp {
        //         return Some(i)
        //     }
        // }
        if let Some(locations) = self.recent_document_locations.locations.get(fp) {
            for path in locations.clone().iter() {
                if let Some(idx) = self.load_file(&Path::new(path)) {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn reload_doc_info(&mut self, path_buf: &PathBuf) {
        match fs::read_to_string(path_buf) {
            Ok(serialized) => match serde_json::from_str(&serialized) {
                Ok(loaded_info) => {
                    let info: DocumentInfo = loaded_info;
                    for doc in &mut self.loaded_documents.iter_mut() {
                        if doc.fingerprint == info.fingerprint {
                            //doc.info = info.clone();
                            doc.generate_reverse_bookmarks(&info);
                            break;
                        }
                    }
                    self.all_local_documents_info
                        .insert(info.fingerprint.clone(), info);
                }
                Err(e) => println!("error reloading doc info: {}", e),
            },
            Err(e) => println!("doc info reloading error: {}", e),
        }
    }

    pub fn save_document_info(&self, fingerprint: &str) -> Option<PathBuf> {
        let data_dir = self.preferences.syncable_data_directory.clone();
        if fs::create_dir_all(&*data_dir).is_err() {
            println!("Unable to create data directory");
            return None;
        }

        let mut path_buf = PathBuf::from(&*data_dir);

        let doc_info = &self.all_local_documents_info.get(fingerprint).unwrap();

        path_buf.push(fingerprint);

        path_buf.set_extension("json");

        let serialized = serde_json::to_string(&doc_info).unwrap();

        if let Err(e) = fs::write(&path_buf, &serialized[..]) {
            println!("Error writing file: {}", e);
        }

        println!("SAVED {:?} {}", path_buf, doc_info.description);
        Some(path_buf)
    }

    fn save_all_doc_data(&mut self) {
        for doc in self.loaded_documents.iter() {
            if doc.doc_info_changed {
                if let Some(path_buf) = self.save_document_info(&doc.fingerprint) {
                    if let Some(s) = path_buf.to_str() {
                        self.just_saved.insert(s.to_string());
                    }
                }
            }
        }

        if let Some(dir) = self.local_data_directory.clone() {
            fs::create_dir_all(&*dir).expect("Unable to create data directory");

            let mut path = PathBuf::from(&*dir);
            path.push("file_locations");
            path.set_extension("json");

            let serialized = serde_json::to_string(&self.recent_document_locations).unwrap();

            if let Err(e) = fs::write(&path, &serialized[..]) {
                println!("Error writing file: {}", e);
            }

            if let Some(s) = path.to_str() {
                self.just_saved.insert(s.to_string());
            }

            let mut prefs_path = PathBuf::from(&*dir);
            prefs_path.push("preferences");
            prefs_path.set_extension("json");

            let serialized = serde_json::to_string(&self.preferences).unwrap();

            if let Err(e) = fs::write(prefs_path, &serialized[..]) {
                println!("Error writing file: {}", e);
            }

            if let Some(s) = path.to_str() {
                self.just_saved.insert(s.to_string());
            }
        }
    }

    // fn make_window(&mut self, filepath: &Path) -> Option<WindowDesc<AppState>> {
    //     if let Some(document_id) = self.load_file(filepath) {
    //         let title = format!("{}", filepath.to_str().expect("unables to make string from file path"));
    //         let new_win = WindowDesc::new(make_pdf_view_window(document_id, None))
    //             .title(title);
    //         return Some(new_win)
    //     }
    //     None
    // }
}

struct Delegate {
    window_count: usize,
    windows_to_open: Vec<usize>,
}

impl Delegate {
    fn new(windows: Vec<usize>) -> Self {
        Self {
            window_count: 0,
            windows_to_open: windows,
        }
    }
}

pub const CHECK_FOR_WINDOWS_TO_OPEN: Selector = Selector::new("check-for-files-to-open");
pub const SYNCABLE_DIRECTORY_FILES_CHANGED: Selector<PathBuf> =
    Selector::new("syncable-director-files-changed");
use crate::pdf_view::SAVE_DOCUMENT_INFO;

impl AppDelegate<AppState> for Delegate {
    fn command(
        &mut self,
        ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut AppState,
        _env: &Env,
    ) -> Handled {
        if let Some(message) = cmd.get(RECEIVED_MESSAGE) {
            let args = message.split('\t');
            for arg in args {
                if let Some(doc_id) = data.load_file(Path::new(arg)) {
                    ctx.new_window(make_pdf_view_window(data, doc_id, None));
                }
            }
            //            println!("MESSAGE! {} {}", message, message.len());
            Handled::Yes
        } else if let Some(file_info) = cmd.get(sys_cmds::OPEN_FILE) {
            if let Some(doc_id) = data.load_file(&file_info.path()) {
                ctx.new_window(make_pdf_view_window(data, doc_id, None));
            }
            Handled::Yes
        } else if let Some(fingerprint) = cmd.get(OPEN_BOOK_WITH_FINGERPRINT) {
            if let Some(doc_idx) = data.open_book_with_fingerprint(fingerprint) {
                ctx.new_window(make_pdf_view_window(data, doc_idx, None));
            }
            Handled::Yes
        } else if let Some(doc_idx) = cmd.get(crate::pdf_view::SHOW_BOOK_INFO) {
            //let pref_win =;
            ctx.new_window(crate::book_info_window::make_book_info_window(
                &data, *doc_idx,
            ));
            Handled::Yes
        } else if let Some(old_view) = cmd.get(NEW_VIEW_WITH_PARENT) {
            ctx.new_window(make_pdf_view_window(
                data,
                old_view.docu_idx,
                Some(old_view.clone()),
            ));
            Handled::Yes
        } else if let Some(fingerprint) = cmd.get(SAVE_DOCUMENT_INFO) {
            //data.reload_doc_info(&doc_idx);
            //            data.loaded_documents[*doc_idx].doc_info_changed = false;
            if let Some(idx) = data
                .loaded_documents
                .iter()
                .position(|doc| doc.fingerprint == *fingerprint)
            {
                data.loaded_documents[idx].doc_info_changed = false;

                // gets modified by the user poking at the pdf_view window
                if let Some(path_buf) = data.save_document_info(&fingerprint) {
                    if let Some(s) = path_buf.to_str() {
                        data.just_saved.insert(s.to_string());
                    }
                }
            } else {
                // gets modified by editing unloaded documents' descriptions in the "load known documents" window
                if let Some(path_buf) = data.save_document_info(&fingerprint) {
                    if let Some(s) = path_buf.to_str() {
                        data.just_saved.insert(s.to_string());
                    }
                }
            }

            Handled::Yes
        } else if let Some(path_buf) = cmd.get(SYNCABLE_DIRECTORY_FILES_CHANGED) {
            if let Some(s) = path_buf.to_str() {
                if data.just_saved.contains(&s.to_string()) {
                    data.just_saved.remove(&s.to_string());
                    return Handled::Yes;
                }
            }
            println!("Reloading, as it's changed on disk: {:?}", &path_buf);
            data.reload_doc_info(&path_buf);
            Handled::Yes
        } else {
            match cmd {
                _ if cmd.is(CHECK_FOR_WINDOWS_TO_OPEN) => {
                    for doc_id in &self.windows_to_open {
                        ctx.new_window(make_pdf_view_window(data, *doc_id, None));
                    }
                    self.windows_to_open.clear();
                    Handled::Yes
                }
                _ if cmd.is(SHOW_PREFERENCES) => {
                    let pref_win = WindowDesc::new(make_preferences_window())
                        .title(LocalizedString::new("PDF Book Reader preferences"));
                    ctx.new_window(pref_win);
                    Handled::Yes
                }
                _ if cmd.is(sys_cmds::NEW_FILE) => {
                    println!("open file");
                    // let new_win = WindowDesc::new(ui_builder())
                    //     .menu(make_menu(data))
                    //     .window_size((data.selected as f64 * 100.0 + 300.0, 500.0));
                    // ctx.new_window(new_win);
                    Handled::Yes
                }
                _ => Handled::No,
            }
        }
    }

    fn window_added(
        &mut self,
        _id: WindowId,
        _data: &mut AppState,
        _env: &Env,
        _ctx: &mut DelegateCtx,
    ) {
        self.window_count += 1;
    }

    fn window_removed(
        &mut self,
        _id: WindowId,
        data: &mut AppState,
        _env: &Env,
        _ctx: &mut DelegateCtx,
    ) {
        self.window_count -= 1;

        if self.window_count == 0 {
            data.save_all_doc_data();
            druid::Application::global().quit();
        }
    }
}

use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};

use std::env;

fn handle_ipc_error(connection: std::io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
    match connection {
        Ok(val) => Some(val),
        Err(error) => {
            panic!("Incoming connection failed: {}", error);
        }
    }
}

pub const RECEIVED_MESSAGE: Selector<String> = Selector::new("received-message");

const IPC_CONNECTION_NAME: &str = "/tmp/pdf-book-reader.sock";

fn listen_for_messages(event_sink: druid::ExtEventSink) {
    let listener = LocalSocketListener::bind(IPC_CONNECTION_NAME).expect(
        "Something went wrong while trying to listen for other instances of `pdf-book-reader`",
    );
    for conn in listener.incoming().filter_map(handle_ipc_error) {
        let mut conn = BufReader::new(conn);
        let mut buffer = String::new();
        conn.read_line(&mut buffer).unwrap();

        if event_sink
            .submit_command(RECEIVED_MESSAGE, buffer.clone(), Target::Auto)
            .is_err()
        {
            break;
        }

        // println!("newcomer asked us to open this/these: {}", buffer);
    }
    println!("no more messages?");
}

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

fn main() -> Result<(), PlatformError> {
    let conn = LocalSocketStream::connect(IPC_CONNECTION_NAME);

    match conn {
        // connected to a preexisting copy of ourselves, send them our command line args then quit
        Ok(mut conn) => {
            let message = env::args().skip(1).collect::<Vec<String>>().join("\t");
            conn.write_all(message.as_bytes()).unwrap();
        }
        Err(_) => {
            let mut state = AppState::new();

            let mut args: Vec<String> = env::args().collect();

            if args.len() == 1 {
                args.push("mupdf_explored.pdf".to_string());
            }

            let windows: Vec<usize> = args
                .iter()
                .skip(1)
                .filter_map(|x| state.load_file(&Path::new(x)))
                //                    .filter(|um| if let Some(..) = um { true } else { false })
                //                  .map(|u| u.unwrap())
                .collect();

            if !state.loaded_documents.is_empty() {
                let launcher = AppLauncher::with_window(make_pdf_view_window(&mut state, 0, None))
                    .delegate(Delegate::new(windows[1..].to_vec()));

                let message_event_sink = launcher.get_external_handle();
                std::thread::spawn(move || listen_for_messages(message_event_sink));

                //                if let Some(dir) = state.syncable_data_directory.clone() {
                //if let Some(dir) = state.preferences.syncable_data_directory.clone() {
                let file_change_notifications_event_sink = launcher.get_external_handle();

                let watcher: Result<RecommendedWatcher, notify::Error> =
                    Watcher::new_immediate(move |res: Result<notify::event::Event, _>| {
                        if let Ok(event) = res {
                            // https://docs.rs/notify/5.0.0-pre.6/notify/event/enum.EventKind.html
                            if let notify::EventKind::Access(notify::event::AccessKind::Close(
                                notify::event::AccessMode::Write,
                            )) = event.kind {
                                for path_buf in event.paths {
                                    if let Err(e) = file_change_notifications_event_sink.submit_command(
                                        SYNCABLE_DIRECTORY_FILES_CHANGED,
                                        Box::<PathBuf>::new(path_buf),
                                        Target::Auto,
                                    ) {
                                        println!("error sending file change notification: {}",e);
                                    }
                                }
                            }
                        }
                    });
                match watcher {
                    Ok(mut w) => {
                        let p = Path::new(&state.preferences.syncable_data_directory);
                        if let Err(e) = w.watch(p, RecursiveMode::NonRecursive) {
                            println!("problem trying to watch syncable data directory: {}", e);
                        }

                        state.filesystem_watcher = Some(Arc::new(w));
                    }
                    Err(e) => println!("error making filesystem watcher: {}", e),
                }

                // let (mut doc_info, _) = DocumentInfo::from_fingerprint(&PathBuf::from(&self.preferences
                //                                                        .syncable_data_directory),
                // }

                launcher.launch(state)?;
            }
        }
    }
    let _ = fs::remove_file(Path::new(IPC_CONNECTION_NAME));
    Ok(())
}
