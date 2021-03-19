// druid-help comment by Lloyd Massiah https://xi.zulipchat.com/#narrow/stream/255910-druid-help/topic/Understanding.20druid.3A.3Awidget.3A.3AScope/near/226690651

//  4:46 PM

// you don't need the mutation checking logic I believe. The im::Hashmap handles that for you.
//   4:48 PM

// The logic is there because you can't mutably change anything behind an Arc, however im structures do allow you to do that. To modify data behind an Arc, you have to clone the data.
//   5:10 PM

// Heres an example of how I used the scope widget

pub fn folder_navigator() -> Box<dyn Widget<AppState>> {
    let navigator = Navigator::new(FolderView::Folder, folder_view_main)
        .with_view_builder(FolderView::SingleImage, image_view_builder);

    let scope = Scope::from_function(
        FolderGalleryState::new,
        GalleryTransfer,
        navigator,
    );

    Box::new(scope)
}

// I scope into my appstate here using the FolderGalleryState::new. GalleryTransfer handles synchronization between AppState and FolderGalleryState.

#[derive(Clone, Data, Lens, Debug)]
pub struct AppState {
    pub folder_paths: HashSet<Arc<PathBuf>>,
    pub current_image_idx: usize,
    pub views: Vector<AppView>,
    pub all_images: Vector<ImageFolder>,
    pub selected_folder: Option<usize>,
}

#[derive(Debug, Clone, Data, Lens)]
pub struct FolderGalleryState {
    pub name: Arc<PathBuf>,
    pub images: Vector<Thumbnail>,
    pub selected_folder: Option<usize>,
    pub selected_image: usize,
    pub views: Vector<FolderView>,
    pub paths: Vector<Arc<PathBuf>>,
}


impl FolderGalleryState {
    pub fn new(state: AppState) -> Self {
        if let Some(idx) = state.selected_folder {
            Self {
                name: state.all_images[idx].name.clone(),
                images: state.all_images[idx].thumbnails.clone(),
                selected_folder: Some(idx),
                selected_image: 0,
                views: vector![FolderView::Folder],
                paths: state.all_images[idx].paths.clone(),
            }
        } else {
            Self {
                name: Arc::new(PathBuf::from("".to_string())),
                images: Vector::new(),
                selected_folder: None,
                selected_image: 0,
                views: vector![FolderView::Folder],
                paths: Vector::new(),
            }
        }
    }
}

// I chose to write my own transfer struct because Lens isn't ergonomic enough to lens into multiple parts of the AppState.

pub struct GalleryTransfer;

impl ScopeTransfer for GalleryTransfer {
    type In = AppState;

    type State = FolderGalleryState;

    fn read_input(&self, state: &mut Self::State, inner: &Self::In) {
        match inner.selected_folder {
            Some(idx) => {
                if let Some(current_idx) = state.selected_folder {
                    if idx != current_idx {
                        let folder = &inner.all_images[idx];
                        dbg!("Change Folder", &folder.name);
                        state.selected_folder = Some(idx);
                        state.name = folder.name.clone();
                        state.images = folder.thumbnails.clone();
                        state.paths = folder.paths.clone();
                    }
                } else {
                    let folder = &inner.all_images[idx];
                    dbg!("None", &folder.name);
                    state.selected_folder = Some(idx);
                    state.name = folder.name.clone();
                    state.images = folder.thumbnails.clone();
                    state.paths = folder.paths.clone();
                }
            }
            None => {
                dbg!("Nothing should be read or maybe it should");
            }
        }
    }

    fn write_back_input(&self, state: &Self::State, inner: &mut Self::In) {
        if let Some(idx) = state.selected_folder {
            inner.all_images[idx].name = state.name.clone();
            inner.all_images[idx].thumbnails = state.images.clone();
        } else {
            dbg!("This should do nothing because there is no state to write back.");
        }
    }
}