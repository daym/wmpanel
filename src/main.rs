use x11rb::atom_manager;
use x11rb::connection::Connection;
//use x11rb::errors::ReplyOrIdError;
use std::path::Path;
use std::borrow::Cow;
use std::process::Command;
use std::collections::HashSet;
use std::fs;
use x11rb::properties::WmHints;
use x11rb::properties::WmHintsState;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::COPY_DEPTH_FROM_PARENT;
use x11rb::rust_connection::RustConnection;
use std::ffi::OsString;

/* TODO:

register for stuff you like (XSelectInput -> ChangeWindowAttributes; as a reaction to XCB_CREATE_NOTIFY)
print events

*/

/*

cookie: Cookies are handles to future replies or errors from the X11 server.
cursor:
errors:
event_loop_integration:
image:
properties: !
protocol:
resource_manager:
rust_connection:
x11_utils:

 */

/*
  shape_mask = XCreatePixmap(display,main_window,wsize,wsize,1);

  if (shape_mask) {
    shape_gc = XCreateGC(display,shape_mask,0,NULL);

    XSetForeground(display,shape_gc,1);
    XFillRectangle(display,shape_mask,shape_gc,0,0,wsize,wsize);

    XShapeCombineMask(display,main_window,ShapeBounding,0,0,shape_mask,ShapeSet);
    XShapeCombineMask(display,icon_window,ShapeBounding,0,0,shape_mask,ShapeSet);

    XFreeGC(display,shape_gc);
    XFreePixmap(display,shape_mask);
  }

*/

atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        UTF8_STRING,
    }
}

fn load_scale_image(name: &Path, target_width: u16, target_height: u16) -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    let img = image::io::Reader::open(name)?.decode()?;
                   // let img2 = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;

    use image::imageops;
    use image::imageops::FilterType;
    // TODO: Keep aspect ratio somehow
    let img = img.resize(target_width.into(), target_height.into(), FilterType::Gaussian);

    /*
    let image: &dyn GenericImageView<Pixel=Rgb<u8>> = &buffer;
    fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&Self>

    Function image::imageops::resize

    pub fn resize<I: GenericImageView>(
        image: &I,
        nwidth: u32,
        nheight: u32,
        filter: FilterType (Gaussian)
    ) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
    where
        I::Pixel: 'static,
        <I::Pixel as Pixel>::Subpixel: 'static,


    */
    Ok(img)
}

fn new_x_image(img: image::DynamicImage) -> Result<x11rb::image::Image<'static>, Box<dyn std::error::Error>> {
    let image_width = u16::try_from(img.width())?;
    let image_height = u16::try_from(img.height())?;
    let mut image_data = img.into_rgba8();
    for (x, y, pixel) in image_data.enumerate_pixels_mut() {
        let image::Rgba(data) = *pixel;
        // apparently, x11rb wants [b, g, r, a] and we have [r, g, b, a].
        if data[3] == 0 {
            *pixel = image::Rgba([0xa0, 0xa0, 0xa0, 255]);  // very good
        } else {
            *pixel = image::Rgba([data[2], data[1], data[0], data[3]]);  // very good
        }
        // *pixel = image::Rgba([0, 0, 100, 255]);  // very good
                           // ^b  ^g ^r  ^ignored
        // *pixel = image::Rgba([data[1], data[2], data[3], data[0]]);
    }
    use image::Rgba;
    //use image::ImageBuffer;
    //let s: ImageBuffer<Rgba<u8>, Vec<u8>> = image_data;
    let image = x11rb::image::Image::new(
        image_width,
        image_height,
        x11rb::image::ScanlinePad::Pad8,
        24, /* depth */
        x11rb::image::BitsPerPixel::B32,
        x11rb::image::ImageOrder::MsbFirst, // no effect
        Cow::Owned(image_data.into_raw()),
    )?;

    /*
    pub fn convert(
        &self,
        scanline_pad: ScanlinePad,
        bits_per_pixel: BitsPerPixel,
        byte_order: ImageOrder
    ) -> Cow<'_, Self>
    */

    // TODO: scale or something. Maybe right after loading it from the file, tho?
    Ok(image)
}

fn create_window(atoms: &AtomCollection, conn: &RustConnection, screen: &Screen, width: u16, height: u16) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    use std::os::unix::ffi::OsStrExt;
    let mainwin_id = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        mainwin_id,
        screen.root,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().background_pixel(screen.white_pixel),
    )?;
    let iconwin_id = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        iconwin_id,
        mainwin_id,
        0,
        0,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new().background_pixel(screen.white_pixel),
    )?;

    // TODO: XSelectInput(display,main_window,event_mask);
    // TODO: XSelectInput(display,icon_window,event_mask);

    let title = std::env::args_os().next().unwrap();

    {
        use x11rb::wrapper::ConnectionExt; // change_property8
                                           //use x11rb::protocol::xproto::ConnectionExt;

        // TODO set WM_COMMAND to argc, argv ?

        conn.change_property8(
            PropMode::REPLACE,
            mainwin_id,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            title.as_bytes(),
        )?;
        conn.change_property8(
            PropMode::REPLACE,
            mainwin_id,
            atoms._NET_WM_NAME,
            atoms.UTF8_STRING,
            title.as_bytes(),
        )?;
        // TODO: maybe only if supported?
        conn.change_property32(
            PropMode::REPLACE,
            mainwin_id,
            atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[atoms.WM_DELETE_WINDOW],
        )?;
        conn.change_property8(
            PropMode::REPLACE,
            mainwin_id,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"simple_window\0DockApp\0",
        )?;
    }

    let mut hints = WmHints::new();
    hints.initial_state = Some(WmHintsState::Iconic);
    hints.icon_window = Some(iconwin_id);
    hints.icon_position = Some((0, 0));
    hints.window_group = Some(mainwin_id);

    // WindowMaker used to be weird...
    // However, nowadays, we can just set WM_CLASS to include "DockApp" and not bother with the other stuff.
    /*
        use x11rb::x11_utils::Serialize;
        let mut hints = hints.serialize();
        // Replace the above WmHintsState::Iconic with Withdrawn.
        // This is a Window-Maker-specific non-standard protocol extension not explicitly supported by x11rb.
        hints[(2*4)..(3*4)].copy_from_slice(&0u32.to_ne_bytes());
        conn.change_property(PropMode::REPLACE, mainwin_id, AtomEnum::WM_HINTS, AtomEnum::WM_HINTS, 32, 9, &hints);
    */

    // Fluxbox:
    //    if (winclient->initial_state == WithdrawnState ||
    //        winclient->getWMClassClass() == "DockApp") {

    hints.set(conn, mainwin_id)?; // TODO .reply_unchecked()? or something
    Ok((mainwin_id, iconwin_id))
}

/*
fonts

    let fonts = x11rb::protocol::xproto::list_fonts(&conn, 10000, b"-misc-fixed-*")
        .unwrap()
        .reply_unchecked()
        .unwrap()
        .unwrap();
    let fonts = fonts
        .names
        .into_iter()
        .map(|strstr| std::str::from_utf8(&strstr.name).unwrap().to_string())
        .collect::<Vec<_>>();
        for font in fonts {
        println!("font {font}");
    }
           pub fn image_text8<'c, 'input, Conn>(
        conn: &'c Conn,
        drawable: Drawable,
        gc: Gcontext,
        x: i16,
        y: i16,
        string: &'input [u8]
    ) -> Result<VoidCookie<'c, Conn>, ConnectionError>
    Fills the destination rectangle with the background pixel from gc, then paints the text with the foreground pixel from gc. The upper-left corner of the filled rectangle is at [x, y - font-ascent]. The width is overall-width, the height is font-ascent + font-descent. The overall-width, font-ascent and font-descent are as returned by xcb_query_text_extents (TODO).
*/

struct Launcher {
    icon_window_id: Window,
    args: Vec<OsString>,
    startup_notify: Option<bool>,
    startup_wm_class: Option<String>,
    // TODO: startup notification flag etc
}

fn create_launcher(atoms: &AtomCollection, conn: &RustConnection, screen: &Screen, gc_id: u32, root: u32, icon_name: &Path, width: u16, height: u16, args: Vec<OsString>, startup_notify: Option<bool>, startup_wm_class: Option<String>) -> Result<Launcher, Box<dyn std::error::Error>> {
    let image = new_x_image(load_scale_image(icon_name, width, height)?)?;
// TODO: image::imageops: blur, brighten, invert
// TODO: See also https://crates.io/crates/imageproc

    let pixmap_id = conn.generate_id().unwrap();
    let depth = screen.root_depth;
    conn.create_pixmap(depth, pixmap_id, root, width, height)
        .unwrap(); // TODO: automatically recreate when depth changes (or size changes--which it shouldn't).

    image.put(conn, pixmap_id, gc_id, 0, 0).unwrap(); // FIXME: if shm, use shm!

    let (mainwin_id, iconwin_id) = create_window(atoms, conn, screen, width, height)?;
    let change = ChangeWindowAttributesAux::default()
        .event_mask(
            EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::ENTER_WINDOW
                | EventMask::LEAVE_WINDOW
                | EventMask::PROPERTY_CHANGE
                | EventMask::RESIZE_REDIRECT
                | EventMask::POINTER_MOTION, //| EventMask::POINTER_MOTION_HINT
        )
        .background_pixmap(Some(pixmap_id));
    //    conn.free_pixmap(pixmap_id).unwrap();
    //    conn.free_gc(gc_id).unwrap();
    // The background pixmap and window must have the same root and same depth
    // it can be destroyed immediately after using it here
    // TODO: background_pixmap, background_pixel, border_pixmap, border_pixel, backing_store = BackingStore::WHEN_MAPPED|ALWAYS, backing_planes, backing_pixel
    let res = conn.change_window_attributes(iconwin_id, &change)?.check();

    let change = ChangeWindowAttributesAux::default().background_pixmap(Some(pixmap_id));
    let res = conn.change_window_attributes(mainwin_id, &change)?.check();

    conn.map_window(mainwin_id)?;
    conn.map_window(iconwin_id)?;
    Ok(Launcher {
        icon_window_id: iconwin_id,
        args: args, // .iter().map(|x| x.to_string()).collect::<Vec<String>>(),
        startup_notify,
        startup_wm_class,
    })
}

/*
let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp").unwrap();

let logo_path = xdg_dirs
    .find_data_file("logo.png")
    .expect("application data not present");
let mut logo_file = File::open(logo_path)?;
let mut logo = Vec::new();
logo_file.read_to_end(&mut logo)?;
pub fn find_data_files<P: AsRef<Path>>(&self, path: P) -> FileFindIterator ⓘ


*/

use std::fs::read_to_string;

fn read_lines(filename: &str) -> Vec<String> {
    read_to_string(filename)
        .unwrap()  // panic on possible file-reading errors
        .lines()  // split the string into an iterator of string slices
        .map(String::from)  // make each slice into a string
        .collect()  // gather them together into a vector
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hidden_desktop_files = read_lines("/home/dannym/.fluxbox/guihidden");
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let atoms = AtomCollection::new(&conn)?.reply()?;
    let screen = &conn.setup().roots[screen_num];
    let width: u16 = 64;
    let height: u16 = 64;
    let root = screen.root;
    let gc_id = conn.generate_id().unwrap();
    let gc_aux = CreateGCAux::new().foreground(screen.white_pixel);
    conn.create_gc(gc_id, root, &gc_aux).unwrap();
    let change = ChangeGCAux::default()
        .foreground(Some(0))
        .fill_style(Some(FillStyle::SOLID)); // TODO: font, subwindow_mode, fill_rule, fill_style
    conn.change_gc(gc_id, &change)?.check();

    //conn.set_foreground(gc_id, 0/*FIXME*/);
    // TODO: conn.poly_fill_rectangle(pixmap_id, gc_id, &[rect]).unwrap();
    //conn.copy_area(pixmap, root, gc, 0, 0, 0, 0, 400, 400).unwrap();
    //conn.flush().unwrap();
    let mut launchers = Vec::<Launcher>::new();

/* crate "xdg"
    let xdg_dirs = xdg::BaseDirectories::new().unwrap();
    let applicationss = xdg_dirs.find_data_files("applications");
    for applications in applicationss {
        for entry in fs::read_dir(applications).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().unwrap() == "desktop" {
                println!("{:?}", entry);
            }
        }
    }

    modules:
    basedir
    categories
    icon_finder
    
*/

    use xdgkit::desktop_entry::DesktopEntry;
    use xdgkit::basedir::{applications};
    let desktop_directories = applications().unwrap();
    let desktop_directories = desktop_directories.split(":").collect::<Vec<&str>>();
    let mut seen_desktop_directories = HashSet::<String>::new();
    for desktop_directory in desktop_directories {
        if desktop_directory == "" { // bug in xdgkit
            continue
        }
        if seen_desktop_directories.contains(desktop_directory) {
            continue
        }
        seen_desktop_directories.insert(desktop_directory.to_string());
        //println!("DESKTOP DIR {:?}", desktop_directory);
        for entry in fs::read_dir(desktop_directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let file_name = file_name.to_str().unwrap().to_string();
                if hidden_desktop_files.contains(&file_name) {
                    continue
                }
            }
            if path.extension().unwrap() == "desktop" {
                let desktop_entry = DesktopEntry::read(fs::read_to_string(&path).unwrap());
                let name = desktop_entry.name;
                if let Some(only_show_in) = desktop_entry.only_show_in {
                    if only_show_in.len() > 0 {
                        continue;
                    }
                }
                /*if let Some(not_show_in) = desktop_entry.not_show_in {
                    let not_show_in = not_show_in.split(";").collect::<Vec<&str>>().filter(|x| x != "").collect::<Vec<&str>>();
                    // FIXME check
                }*/
                // TODO: try_exec
                let terminal = desktop_entry.terminal;
                let startup_notify = desktop_entry.startup_notify;
                let startup_wm_class = desktop_entry.startup_wm_class;

                let icon = match desktop_entry.icon {
                    None => None,
                    Some(icon_name) => {
                        //println!("icon_name {}", icon_name);
                        let mut result = xdgkit::icon_finder::find_icon(icon_name.to_string(), 64, 1);
                        /*if result.is_none() {
                            result = xdgkit::icon_finder::find_icon(icon_name.to_string() + "-symbolic", 64, 1);
                            if result.is_some() {
                                eprintln!("-symbolic");
                            }
                        }*/
                        result
                    },
                };
                match desktop_entry.exec {
                    None => {
                    }
                    Some(ref command_line) => {
                        let args = command_line
                        .split(" ")
                        .flat_map(|x| match x {
                           "%F" | "%f" | "%u" | "%U" => vec![],
                           // Deprecated
                           "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => vec![],
                           "%k" => vec![OsString::from(path.clone())],
                           "%c" => vec![OsString::from(name.clone().unwrap_or_default())],
                           "%i" => vec![OsString::from("--icon"), icon.clone().unwrap_or_default().into()],
                           x => vec![OsString::from(x)],
                        })
                        .collect::<Vec<OsString>>();
                        
                        let icon_path = match icon {
                            Some(x) => {
                                if x == Path::new("").to_path_buf() { // XXX
                                    Path::new("printer.png").to_path_buf()
                                } else {
                                    x
                                }
                            },
                            None => Path::new("printer.png").to_path_buf(),
                        };
                        //println!("ICON PATH {:?}", icon_path);
                        let launcher = create_launcher(&atoms, &conn, &screen, gc_id, root, &icon_path, width, height, args, startup_notify, startup_wm_class);
                        match launcher {
                            Ok(launcher) => launchers.push(launcher),
                            Err(x) => {
                                eprintln!("{:?}", x);
                            }
                        }
                    }
                }
            }
        }
    }
    let launchers = &launchers;

    conn.flush();
    loop {
        let event = conn.wait_for_event()?;
        println!("Event: {:?}", event);
        match event {
            Event::ButtonRelease(x) => {
                use std::os::unix::process::CommandExt;
                let hostname = hostname::get().unwrap();
                if x.detail == 1 { // x.state.contains(KeyButMask::BUTTON1) {
                    let time = x.time;
                    let window_id = x.event;
                    let launcher = launchers.iter().find(|&launcher| launcher.icon_window_id == window_id).unwrap();
                    let args = launcher.args.clone();
                    let startup_notify = launcher.startup_notify;
                    unsafe {
                        let error = Command::new("env")
                            .pre_exec(move || {
                                use arrform::{arrform, ArrForm};
                                let err = if startup_notify.is_some() && startup_notify.unwrap() {
                                    let pid = std::process::id();
                                    // See <https://cgit.freedesktop.org/startup-notification/tree/doc/startup-notification.txt>
                                    let desktop_startup_id = arrform!(
                                        280,
                                        "DESKTOP_STARTUP_ID={:?}+{}+_TIME{}",
                                        hostname,
                                        pid,
                                        time
                                    );
                                    exec::Command::new("env")
                                    .arg(desktop_startup_id.as_str().to_string())
                                    .args(&args)
                                    .exec()
                                } else {
                                    exec::Command::new("env")
                                    .args(&args)
                                    .exec()
                                };
                                // TODO: on launchee startup failure, we should treat the launch sequence as ended and we send the "end" message ourselves.
                                Err(match err {
                                    exec::Error::BadArgument(e) => {
                                        panic!("bad argument")
                                    }
                                    exec::Error::Errno(e) => std::io::Error::from(e),
                                })
                            })
                            .spawn();
                    }
                }
            }
            _ => {}
        }
    }
}
