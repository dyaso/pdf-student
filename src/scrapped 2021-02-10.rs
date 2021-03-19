use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
// use interprocess::local_socket::LocalSocketStream;
use std::{
    env,
    error::Error,
    io::{self, prelude::*, BufReader},
};

use mupdf::pdf::PdfDocument;
use mupdf::{Colorspace, Matrix};

use std::collections::BTreeMap;
use std::rc::Rc;
use std::cell::RefCell;

use druid::kurbo::BezPath;
use druid::piet::{FontFamily, ImageFormat, InterpolationMode, Text, TextLayoutBuilder};

use druid::widget::prelude::*;
use druid::{AppDelegate, AppLauncher, Command, DelegateCtx, Handled, Target, WindowDesc, WindowId,
Affine, Point, FontWeight, FontStyle, TextLayout, Rect, Color, FontDescriptor, Selector


,MouseButton, KeyEvent
};
use druid::Target::Global;
use druid::widget::{Label};




use path_slash::PathExt;


//use log::info;

const NEW_WINDOW_ACTION: Selector = Selector::new("pdfprogress-new-window");

type WindowPositions = BTreeMap < WindowId, String >;
type Buffers = BTreeMap < String, String >;

//denum { Display, Crop, Overview }

struct Delegate {
//    window_count: u32,
}
// C:\\Windows\\system32\\WindowsPowerShell\\v1.0\\powershell.exe -NoExit -Command Set-Location -LiteralPath '%L'
#[derive(Clone, Debug, Default, Data)]
struct State {
    windows: Rc < RefCell < WindowPositions > >,
    documents: Rc < RefCell < WindowPositions > >,
    //buffers: Rc<Buffers>,
}

impl AppDelegate<State> for Delegate {
    fn command(
        &mut self,
        ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &Command,
        data: &mut State,
        _env: &Env,
    ) -> Handled {
        match cmd {
            _ if cmd.is(NEW_WINDOW_ACTION) => {
                println!("NEW WINODW");
                let new_win = WindowDesc::new(|| {WorkspaceWidget})
                        //.set_window_state(data.clone())
                        ;
                ctx.new_window(new_win);

                Handled::Yes
            },
            _ => Handled::No,
        }
    }

    fn window_added(
        &mut self,
        id: WindowId,
        data: &mut State,
        _env: &Env,
        _ctx: &mut DelegateCtx,
    ) {
//        let mut windows = .borrow_mut();
        data.windows.borrow_mut().entry(id).or_insert(format!("floop {:?}", id));
    //let (first_key, first_value) = map.iter().next().unwrap();
        println!("Window added, id: {:?} \n{:?}\n", id, data);
    }

    fn window_removed(
        &mut self,
        id: WindowId,
        _data: &mut State,
        _env: &Env,
        _ctx: &mut DelegateCtx,
    ) {
        println!("Window removed, id: {:?}", id);
    }
}

fn ui() -> impl Widget<State> {
    let text = "Hello";//LocalizedString::new("hello-counter");
    let label = Label::new(text);
    label
}

struct WorkspaceWidget;


// If this widget has any child widgets it should call its event, update and layout
// (and lifecycle) methods as well to make sure it works. Some things can be filtered,
// but a general rule is to just pass it through unless you really know you don't want it.
impl Widget<State> for WorkspaceWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut State, _env: &Env) {
        match event {
            Event::WindowConnected => {
                ctx.request_focus();
            },

            Event::MouseDown(e) => {
                if e.button == MouseButton::Left {
                    ctx.submit_command(NEW_WINDOW_ACTION.to(Global));

                    println!("mouse clochk");
                    // data.drawing = true;
                    // let grid_pos_opt = self.grid_pos(e.pos);
                    // grid_pos_opt
                    //     .iter()
                    //     .for_each(|pos| data.grid[*pos] = !data.grid[*pos]);
                }
            },

            Event::KeyDown(e) => {
                println!("{:?} key event {:?}", ctx.window_id(), e);
            },
            _ => ()
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &State,
        _env: &Env,
    ) {
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &State, _data: &State, _env: &Env) {}

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &State,
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
    fn paint(&mut self, ctx: &mut PaintCtx, data: &State, env: &Env) {

        println!("um {:?}", data.windows.borrow().get(&ctx.window_id()));
        // Clear the whole widget with the color of your choice
        // (ctx.size() returns the size of the layout rect we're painting in)
        // Note: ctx also has a `clear` method, but that clears the whole context,
        // and we only want to clear this widget's area.
        let size = ctx.size();
        let rect = size.to_rect();
        ctx.fill(rect, &Color::WHITE);

        // We can paint with a Z index, this indicates that this code will be run
        // after the rest of the painting. Painting with z-index is done in order,
        // so first everything with z-index 1 is painted and then with z-index 2 etc.
        // As you can see this(red) curve is drawn on top of the green curve
        ctx.paint_with_z_index(1, move |ctx| {
            let mut path = BezPath::new();
            path.move_to((0.0, size.height));
            path.quad_to((40.0, 50.0), (size.width, 0.0));
            // Create a color
            let stroke_color = Color::rgb8(128, 0, 0);
            // Stroke the path with thickness 1.0
            ctx.stroke(path, &stroke_color, 5.0);
        });

        // Create an arbitrary bezier path
        let mut path = BezPath::new();
        path.move_to(Point::ORIGIN);
        path.quad_to((40.0, 50.0), (size.width, size.height));
        // Create a color
        let stroke_color = Color::rgb8(0, 128, 0);
        // Stroke the path with thickness 5.0
        ctx.stroke(path, &stroke_color, 5.0);

        // Rectangles: the path for practical people
        let rect = Rect::from_origin_size((10.0, 10.0), (100.0, 100.0));
        // Note the Color:rgba8 which includes an alpha channel (7F in this case)
        let fill_color = Color::rgba8(0x00, 0x00, 0x00, 0x7F);
        ctx.fill(rect, &fill_color);

        // Text is easy; in real use TextLayout should either be stored in the
        // widget and reused, or a label child widget to manage it all.
        // This is one way of doing it, you can also use a builder-style way.
        let mut layout = TextLayout::<String>::from_text("hello");//(data);
        layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(24.0)
            .with_weight(FontWeight::BOLD)
            .with_style(FontStyle::Italic)
            );
//Character("n") mods: Modifiers((empty)) repeat: false
        layout.set_text_color(fill_color);
//        layout.rebuild_if_needed(ctx.text(), env);

        // Let's rotate our text slightly. First we save our current (default) context:
        ctx.with_save(|ctx| {
            // Now we can rotate the context (or set a clip path, for instance):
            // This makes it so that anything drawn after this (in the closure) is
            // transformed.
            // The transformation is in radians, but be aware it transforms the canvas,
            // not just the part you are drawing. So we draw at (80.0, 40.0) on the rotated
            // canvas, this is NOT the same position as (80.0, 40.0) on the original canvas.
            ctx.transform(Affine::rotate(std::f64::consts::FRAC_PI_4));
            layout.draw(ctx, (80.0, 40.0));
        });
        // When we exit with_save, the original context's rotation is restored

        // // This is the builder-style way of drawing text.
        // let text = ctx.text();
        // let layout = text
        //     .new_text_layout("hi")//data.clone())
        //     .font(FontFamily::SERIF, 24.0)
        //     .text_color(Color::rgb8(128, 0, 0))
        //     .build()
        //     .unwrap();
        // ctx.draw_text(&layout, (100.0, 25.0));

    }
}


fn try_to_open_things(current_dir: String, things: Vec<String>) {

}

fn fingerprint(document: &PdfDocument) -> String {
// fingerprinting PDFs https://www.seanh.cc/2017/11/22/pdf-fingerprinting/
// todo: fall back to MD5 checksumming first 1K of file if it doesn't have an ID string
    let trailer = document.trailer().unwrap();
    let pdf_id = trailer.get_dict("ID").unwrap().unwrap().resolve().unwrap().unwrap().get_array(0).unwrap().unwrap().to_string();
    pdf_id[1 .. pdf_id.len() - 1].to_string()
}


fn main() {//-> Result<(), Box<dyn Error>> {

    let current_dir = env::current_dir().expect("The 'current directory' does not exist, or you have insufficient permissions to access it.");

    let args = env::args().skip(1).collect::<Vec<String>>().join("\t");

    let cd_args = format!("{}\t{}", current_dir.to_str().unwrap().to_string(), args);

    let mut aargs = cd_args.split('\t').into_iter();

    let cd = aargs.next().unwrap();
    //let aaargs = aargs.skip(1);

//    println!("args {:?}", aargs);

    println!("current dir: {}", cd);

    for a in aargs {
        println!("arg {}", a);
    }


//    let path = std::path::Path::new(r"C:\Users\user\Desktop\art books\Andrew Loomis - Creative.Illustration 290.pdf").to_slash().unwrap();
   let path = std::path::Path::new(r"/home/oliver/books/art/illustration/Andrew Loomis - Creative.Illustration 290.pdf").to_slash().unwrap();
    // let path = std::path::Path::new(r"/home/oliver/books/rust/Rust_in_Action_v15.pdf").to_slash().unwrap();

    let document = PdfDocument::open(&path).unwrap();

    // let trailer = document.trailer().unwrap();


    // let pdf_id = trailer.get_dict("ID").unwrap().unwrap().resolve().unwrap().unwrap().get_array(0).unwrap().unwrap().to_string();
    let id_string = fingerprint(&document);//pdf_id[1 .. pdf_id.len() - 1].to_string();

    println!("trailer: {} {}", id_string, id_string.len());


//    let document = PdfDocument::open("C:\\Users\\user\\Desktop\\art books\\Andrew Loomis - Creative.Illustration 290.pdf").unwrap();
//    let document = PdfDocument::open("C:\Users\user\Desktop\art books\Andrew Loomis - Creative.Illustration 290.pdf").unwrap();
    // let document = PdfDocument::open("C:\Users\user\Desktop\art books\Andrew Loomis - Creative.Illustration 290.pdf").unwrap();

//     let path = std::path::PathBuf::from("C:\Users\user\Desktop\art books\Andrew Loomis - Creative.Illustration 290.pdf");

//     println!("Path {:?}", path);

//    let document = PdfDocument::open("mupdf_explored.pdf").unwrap();
    // let document = PdfDocument::open(path.to_str()).unwrap();

    // let contents = document.outlines().expect("Unable to get table of contents");
    // println!("contents count {}", contents.len());

    // let page = document.load_page(3).unwrap();
    
    // let rect = page.bounds().unwrap();
    // println!("page bounds {:?}", rect);

    // let matrix = Matrix::new_scale(72f32 / 72f32, 72f32 / 72f32);
    // let pixmap = page
    //     .to_pixmap(&matrix, &Colorspace::device_rgb(), 0.0, true)
    //     .unwrap();
    // pixmap
    //     .save_as("test.png", mupdf::ImageFormat::PNG)
    //     .unwrap();



    let first_window_state = State::default();

    let first_window = WindowDesc::new(|| {WorkspaceWidget});

    AppLauncher::with_window(first_window)
        .delegate(Delegate {})
        .launch(first_window_state)
        .expect("First window failed to launch");



//     let mut conn = LocalSocketStream::connect("/tmp/pdfprogress.sock");

//     match conn {
//         // connected to a preexisting copy of ourselves, send them our command line args then quit
//         Ok(mut conn) => {

//             let args: Vec<String> = env::args().collect();
//             // println!("My path is {}.", args[0]);
//             // println!("I got {:?} arguments: {:?}.", args.len() - 1, &args[1..]);
//             let mut message : String = "meeble beep\n".to_string();

   //             //message.push_str(format!("message from newcomer: I got {:?} arguments: {:?}.\n", args.len() - 1, &args[1..]).as_str());


//             conn.write_all(message.as_bytes()).unwrap();
//         },
//         Err(err) => {
//             fn handle_error(connection: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
//                 match connection {
//                     Ok(val) => Some(val),
//                     Err(error) => {
//                         panic!("Incoming connection failed: {}", error);
//                     }
//                 }
//             }

//             println!("running in server mode");

// //             let args: Vec<String> = env::args().collect();
// //             let mut message : String = args[0].as_str().to_string();
// //             message.push_str(format!("I got {:?} arguments: {:?}.", args.len() - 1, &args[1..]).as_str());

//             let listener = LocalSocketListener::bind("/tmp/pdfprogress.sock").expect("Something went wrong while trying to listen for other instances of `pdfprogress`");
//             for mut conn in listener.incoming().filter_map(handle_error) {
//                 let mut conn = BufReader::new(conn);
//                 let mut buffer = String::new();
//                 conn.read_line(&mut buffer).unwrap();

//                 println!("newcomer asked us to open this/these: {}", buffer);
//             }

//         },
//     };

}