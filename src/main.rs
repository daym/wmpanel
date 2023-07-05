use x11rb::atom_manager;
use x11rb::connection::Connection;
//use x11rb::errors::ReplyOrIdError;
use std::borrow::Cow;
use std::process::Command;
use x11rb::properties::WmHints;
use x11rb::properties::WmHintsState;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::COPY_DEPTH_FROM_PARENT;

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

fn load_scale_image(target_width: u16, target_height: u16) -> image::DynamicImage {
    let img = image::io::Reader::open("idea.png")
        .unwrap()
        .decode()
        .unwrap(); // into_rgba8()
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
    img
}

fn new_x_image(img: image::DynamicImage) -> x11rb::image::Image<'static> {
    let image_width = u16::try_from(img.width()).unwrap();
    let image_height = u16::try_from(img.height()).unwrap();
    let image_data = img.into_rgba8();
    let image = x11rb::image::Image::new(
        image_width,
        image_height,
        x11rb::image::ScanlinePad::Pad8,
        24, /* depth */
        x11rb::image::BitsPerPixel::B32,
        x11rb::image::ImageOrder::MsbFirst,
        Cow::Owned(image_data.into_raw()),
    )
    .unwrap();

    /*
    pub fn convert(
        &self,
        scanline_pad: ScanlinePad,
        bits_per_pixel: BitsPerPixel,
        byte_order: ImageOrder
    ) -> Cow<'_, Self>
    */

    // TODO: scale or something. Maybe right after loading it from the file, tho?
    image
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::ffi::OsStrExt;
    let (conn, screen_num) = x11rb::connect(None).unwrap();
    let atoms = AtomCollection::new(&conn)?.reply()?;

    let screen = &conn.setup().roots[screen_num];
    let mainwin_id = conn.generate_id()?;
    let width: u16 = 64;
    let height: u16 = 64;
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

    hints.set(&conn, mainwin_id)?; // TODO .reply_unchecked()? or something
    let depth = screen.root_depth;
    let root = screen.root;
    let pixmap_id = conn.generate_id().unwrap();
    conn.create_pixmap(depth, pixmap_id, root, width, height)
        .unwrap(); // TODO: automatically recreate when depth changes (or size changes--which it shouldn't).
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

    //use std::io::Cursor;
    // TODO: scale
    let img = load_scale_image(width, height);
    let image = new_x_image(img);
    image.put(&conn, pixmap_id, gc_id, 0, 0).unwrap(); // FIXME: if shm, use shm!

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
    /*    for font in fonts {
        println!("font {font}");
    }*/
    /*
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

    conn.map_window(mainwin_id)?;
    conn.map_window(iconwin_id)?;
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
                    unsafe {
                        let error = Command::new("gedit")
                            .arg("hello")
                            .pre_exec(move || {
                                use arrform::{arrform, ArrForm};
                                let pid = std::process::id();
                                // See <https://cgit.freedesktop.org/startup-notification/tree/doc/startup-notification.txt>
                                let desktop_startup_id = arrform!(
                                    280,
                                    "DESKTOP_STARTUP_ID={:?}+{}+_TIME{}",
                                    hostname,
                                    pid,
                                    time
                                );
                                use exec::execvp;
                                let err = execvp(
                                    "env",
                                    &["env", desktop_startup_id.as_str(), "gedit", "hello"],
                                );
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
                // TODO: on launchee startup failure, we should treat the launch sequence as ended and we send the "end" message ourselves.
                //child.wait();
                // detach from child
            }
            _ => {}
        }
    }
}
