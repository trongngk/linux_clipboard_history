use crate::db::{self, Entry};
use crate::util::*;
use gtk::gdk;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;
use gtk::prelude::*;
use rusqlite::Connection;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const W: i32 = 470;
const H: i32 = 560;
const RENDER_CAP: usize = 80;

/// Font stack: prefer Inter (free, ships in the `fonts-inter` package and is
/// the closest match to Apple's SF), fall back to SF Pro for users who already
/// have it, then whatever the system provides. Apple does not license SF for
/// redistribution on Linux, so it cannot be bundled here - install Inter for
/// the intended look:  sudo apt install fonts-inter
const FONT_UI: &str = "\"Inter\", \"Inter Variable\", \"SF Pro Text\", \"SF Pro Display\", \"Cantarell\", \"Helvetica Neue\", sans-serif";
const FONT_MONO: &str = "\"JetBrains Mono\", \"SF Mono\", \"Menlo\", \"DejaVu Sans Mono\", monospace";

/// Light theme modelled on the macOS Clipboard panel: translucent background
/// with a large corner radius, flat rows separated by hairlines, and a soft
/// tint for the selected row rather than a solid fill.
const CSS_LIGHT: &str = "
window                { background-color: transparent; }
.panel                { background-color: rgba(243,243,246,__ALPHA__);
                        border-radius: 18px;
                        border: 1px solid rgba(0,0,0,0.10);
                        box-shadow: 0 20px 55px rgba(0,0,0,0.30),
                                    0 3px 10px rgba(0,0,0,0.14); }
.panel-square         { border-radius: 0px; box-shadow: none; }
.header               { border-bottom: 1px solid rgba(0,0,0,0.08); }
entry.search          { background: none; background-color: transparent;
                        border: none; box-shadow: none; padding: 0px;
                        color: #1d1d1f; font-size: 132%; font-weight: 500;
                        caret-color: #007aff; }
entry.search:focus    { background: none; box-shadow: none; }
list                  { background-color: transparent; padding: 4px 2px; }
list > row            { background-color: #ffffff; padding: 0px;
                        border: 1px solid rgba(0,0,0,0.08);
                        border-radius: 12px; margin: 4px 6px;
                        transition: background-color 120ms ease, border-color 120ms ease; }
list > row:hover      { background-color: #ffffff; border-color: rgba(0,0,0,0.16); }
list > row:selected   { background-color: rgba(0,122,255,0.10);
                        border-color: rgba(0,122,255,0.60); }
list > row:selected:hover { background-color: rgba(0,122,255,0.13);
                        border-color: rgba(0,122,255,0.72); }
.rowbox               { padding: 8px 10px 8px 13px; }
.rowtitle             { color: #1d1d1f; font-size: 97%; }
.rowimg               { border-radius: 7px; }
.rowsub               { color: #86868b; font-size: 80%; }
.rowsub-pin           { color: #007aff; font-size: 80%; font-weight: 600; }
.foot                 { color: #8a8a90; font-size: 76%;
                        border-top: 1px solid rgba(0,0,0,0.07); }
.circlebtn            { background-color: rgba(0,0,0,0.05); border: none;
                        border-radius: 9px; color: #8e8e93;
                        padding: 0px 2px; min-height: 20px; min-width: 20px;
                        transition: background-color 120ms ease, color 120ms ease; }
.headbtn              { background-color: rgba(0,0,0,0.06); border: none;
                        border-radius: 13px; color: #6e6e73;
                        padding: 3px 7px; min-height: 26px; min-width: 26px;
                        transition: background-color 120ms ease, color 120ms ease; }
.headbtn:hover        { background-color: rgba(0,122,255,0.16); color: #007aff; }
.circlebtn:hover      { background-color: rgba(0,122,255,0.16); color: #007aff; }
.circlebtn-on         { background-color: rgba(0,122,255,0.18); color: #007aff; }
scrollbar             { background-color: transparent; border: none; }
scrollbar slider      { background-color: rgba(0,0,0,0.20); border-radius: 6px;
                        min-width: 7px; transition: background-color 120ms ease; }
scrollbar slider:hover { background-color: rgba(0,0,0,0.36); }
";

const CSS_DARK: &str = "
window                { background-color: transparent; }
.panel                { background-color: rgba(32,32,34,__ALPHA__);
                        border-radius: 18px;
                        border: 1px solid rgba(255,255,255,0.14);
                        box-shadow: 0 20px 55px rgba(0,0,0,0.55),
                                    0 3px 10px rgba(0,0,0,0.40); }
.panel-square         { border-radius: 0px; box-shadow: none; }
.header               { border-bottom: 1px solid rgba(255,255,255,0.10); }
entry.search          { background: none; background-color: transparent;
                        border: none; box-shadow: none; padding: 0px;
                        color: #f2f2f7; font-size: 132%; font-weight: 500;
                        caret-color: #0a84ff; }
entry.search:focus    { background: none; box-shadow: none; }
list                  { background-color: transparent; padding: 4px 2px; }
list > row            { background-color: rgba(255,255,255,0.07); padding: 0px;
                        border: 1px solid rgba(255,255,255,0.09);
                        border-radius: 12px; margin: 4px 6px;
                        transition: background-color 120ms ease, border-color 120ms ease; }
list > row:hover      { background-color: rgba(255,255,255,0.11);
                        border-color: rgba(255,255,255,0.16); }
list > row:selected   { background-color: rgba(10,132,255,0.24);
                        border-color: rgba(10,132,255,0.70); }
list > row:selected:hover { background-color: rgba(10,132,255,0.30);
                        border-color: rgba(10,132,255,0.85); }
.rowbox               { padding: 8px 10px 8px 13px; }
.rowtitle             { color: #f2f2f7; font-size: 97%; }
.rowimg               { border-radius: 7px; }
.rowsub               { color: #98989d; font-size: 80%; }
.rowsub-pin           { color: #0a84ff; font-size: 80%; font-weight: 600; }
.foot                 { color: #98989d; font-size: 76%;
                        border-top: 1px solid rgba(255,255,255,0.09); }
.circlebtn            { background-color: rgba(255,255,255,0.09); border: none;
                        border-radius: 9px; color: #98989d;
                        padding: 0px 2px; min-height: 20px; min-width: 20px;
                        transition: background-color 120ms ease, color 120ms ease; }
.headbtn              { background-color: rgba(255,255,255,0.10); border: none;
                        border-radius: 13px; color: #c7c7cc;
                        padding: 3px 7px; min-height: 26px; min-width: 26px;
                        transition: background-color 120ms ease, color 120ms ease; }
.headbtn:hover        { background-color: rgba(10,132,255,0.30); color: #ffffff; }
.circlebtn:hover      { background-color: rgba(10,132,255,0.30); color: #ffffff; }
.circlebtn-on         { background-color: rgba(10,132,255,0.32); color: #ffffff; }
scrollbar             { background-color: transparent; border: none; }
scrollbar slider      { background-color: rgba(255,255,255,0.22); border-radius: 6px;
                        min-width: 7px; transition: background-color 120ms ease; }
scrollbar slider:hover { background-color: rgba(255,255,255,0.38); }
";

#[derive(Clone)]
struct Ctx {
    con: Rc<Connection>,
    all: Rc<RefCell<Vec<Entry>>>,
    shown: Rc<RefCell<Vec<Entry>>>,
    result: Rc<RefCell<Option<Entry>>>,
    list: gtk::ListBox,
    search: gtk::Entry,
    foot: gtk::Label,
    pix: Rc<RefCell<HashMap<String, Pixbuf>>>,
}

pub fn pick(con: Rc<Connection>, entries: Vec<Entry>, blur_close: bool) -> Option<Entry> {
    if gtk::init().is_err() {
        eprintln!("[{APP}] could not initialise GTK (is DISPLAY set?)");
        return None;
    }
    load_css();

    let win = gtk::Window::new(gtk::WindowType::Toplevel);
    win.set_title("Clipboard history");
    win.set_decorated(false);
    win.set_resizable(false);
    win.set_keep_above(true);
    win.set_skip_taskbar_hint(true);
    win.set_skip_pager_hint(true);
    win.set_type_hint(gdk::WindowTypeHint::Dialog);
    // set_default_size is ignored for a non-resizable window: GTK uses the
    // natural size of the content instead. set_size_request forces it.
    win.set_default_size(W, H);
    win.set_size_request(W, H);

    // Real rounded corners need an RGBA visual plus app-paintable, otherwise
    // the four corners show the old rectangle underneath. With no compositor
    // available, fall back to square corners.
    let composited = match gdk::Screen::default() {
        Some(screen) => {
            if let Some(visual) = screen.rgba_visual() {
                win.set_visual(Some(&visual));
            }
            screen.is_composited()
        }
        None => false,
    };
    win.set_app_paintable(true);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.style_context().add_class("panel");
    if !composited {
        root.style_context().add_class("panel-square");
    }
    // With a compositor, leave a transparent gutter around the panel so its
    // drop shadow has room to render; the window size stays W x H either way.
    // Without one there is no shadow (and square corners), so use no gutter.
    root.set_margin(if composited { 14 } else { 0 });
    win.add(&root);

    // ---- header: icon + inline search field + ••• button (macOS panel) ----
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.style_context().add_class("header");
    head.set_margin_top(0);
    head.set_border_width(0);
    let head_pad = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head_pad.set_margin_top(11);
    head_pad.set_margin_bottom(10);
    head_pad.set_margin_start(14);
    head_pad.set_margin_end(12);

    let logo = gtk::Image::from_icon_name(Some("edit-paste"), gtk::IconSize::LargeToolbar);
    head_pad.pack_start(&logo, false, false, 0);

    // The search field doubles as the title: typing filters, as on macOS.
    let search = gtk::Entry::new();
    search.style_context().add_class("search");
    search.set_placeholder_text(Some("Clipboard"));
    search.set_has_frame(false);
    head_pad.pack_start(&search, true, true, 0);

    let clear = gtk::Button::with_label("•••");
    clear.style_context().add_class("headbtn");
    clear.set_relief(gtk::ReliefStyle::None);
    clear.set_tooltip_text(Some("Clear all (pinned entries are kept)"));
    head_pad.pack_end(&clear, false, false, 0);

    head.pack_start(&head_pad, true, true, 0);
    root.pack_start(&head, false, false, 0);

    // ---- list ----
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Browse);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .margin_start(12)
        .margin_end(6)
        .build();
    scroll.add(&list);
    root.pack_start(&scroll, true, true, 0);

    let foot = gtk::Label::new(None);
    foot.style_context().add_class("foot");
    foot.set_xalign(0.0);
    foot.set_margin_top(7);
    foot.set_margin_bottom(9);
    foot.set_margin_start(14);
    foot.set_margin_end(14);
    root.pack_start(&foot, false, false, 0);

    let ctx = Ctx {
        con,
        all: Rc::new(RefCell::new(entries)),
        shown: Rc::new(RefCell::new(Vec::new())),
        result: Rc::new(RefCell::new(None)),
        list: list.clone(),
        search: search.clone(),
        foot: foot.clone(),
        pix: Rc::new(RefCell::new(HashMap::new())),
    };

    rebuild(&ctx, 0);

    // ---- signals ----
    {
        let ctx = ctx.clone();
        search.connect_changed(move |_| rebuild(&ctx, 0));
    }
    {
        let ctx = ctx.clone();
        search.connect_activate(move |_| accept(&ctx));
    }
    {
        let ctx = ctx.clone();
        list.connect_row_activated(move |_, row| {
            select_index(&ctx, row.index());
            accept(&ctx);
        });
    }
    {
        let ctx = ctx.clone();
        clear.connect_clicked(move |_| {
            let _ = db::wipe(&ctx.con, false);
            *ctx.all.borrow_mut() = db::fetch(&ctx.con, 300).unwrap_or_default();
            rebuild(&ctx, 0);
        });
    }
    {
        let ctx = ctx.clone();
        win.connect_key_press_event(move |_, ev| on_key(&ctx, ev));
    }

    win.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });

    // Pressing Super+V again: the new instance sends SIGTERM, closing this
    // panel — the same toggle behaviour as Win+V.
    glib::unix_signal_add_local(libc::SIGTERM, || {
        gtk::main_quit();
        glib::ControlFlow::Break
    });

    // Close when the user clicks outside. Two guards against closing wrongly:
    //  1. only armed after 600ms, i.e. once the panel has really taken focus
    //  2. focus-out also fires when focus moves to a child popup and comes
    //     straight back, so wait another 200ms and check is_active() first
    let ready = Rc::new(Cell::new(false));
    let grabbed_check: Rc<RefCell<Option<gdk::Seat>>> = Rc::new(RefCell::new(None));
    if blur_close {
        let ready = ready.clone();
        let grabbed = grabbed_check.clone();
        win.connect_focus_out_event(move |w, _| {
            // When the grab succeeded, button-press already handles clicks
            // outside, so acting on focus-out here would only close wrongly.
            if ready.get() && grabbed.borrow().is_none() {
                let w = w.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    if !w.is_active() {
                        gtk::main_quit();
                    }
                });
            }
            glib::Propagation::Proceed
        });
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
        ready.set(true);
    });

    win.show_all();
    place_near_pointer(&win);
    win.present();
    search.grab_focus();

    // Grab pointer and keyboard for the panel, the same way GTK does for menus
    // and popovers. With owner_events=true the child widgets still receive
    // events normally, while a click outside the panel is delivered to the
    // panel itself with coordinates outside its frame — that is the signal to
    // close. This does not depend on the window manager granting focus, which
    // makes it more reliable than focus-out-event.
    //
    // show_all() only queues the map request; X has not mapped the window yet,
    // so grabbing here returns NotViewable. Retry once the main loop runs.
    // On Wayland a GDK seat grab on a normal toplevel does not deliver clicks
    // that land outside the window (only xdg-popup surfaces may grab), yet it
    // can still report success and thereby suppress the focus-out fallback,
    // leaving the panel stuck open when the user clicks away. Skip the grab
    // there and let the focus-out handler above close the panel instead.
    let seat_holder = grabbed_check.clone();
    if blur_close && !is_wayland() {
        let w = win.clone();
        let h = seat_holder.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(40), move || {
            try_grab(w, h, 0)
        });

        win.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        win.connect_button_press_event(move |w, ev| {
            // Use ROOT coordinates rather than ev.position(): position() is
            // relative to the GdkWindow that received the event, and a click
            // inside the panel may land on a child viewport, so comparing it
            // against the toplevel size would be wrong.
            let (rx, ry) = ev.root();
            let (ox, oy) = w.position();
            let (ww, wh) = (w.allocated_width(), w.allocated_height());
            let outside = rx < ox as f64
                || ry < oy as f64
                || rx > (ox + ww) as f64
                || ry > (oy + wh) as f64;
            if std::env::var_os("CLIPHIST_DEBUG").is_some() {
                eprintln!(
                    "[{APP}] click root=({:.0},{:.0}) panel=({},{} {}x{}) outside={}",
                    rx, ry, ox, oy, ww, wh, outside
                );
            }
            if outside {
                gtk::main_quit();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }

    // Safety net: the panel grabs both pointer and keyboard, so any bug that
    // hangs it would lock the desktop out of input until the process dies.
    // Close automatically after two minutes — no reason to stay open longer.
    if blur_close {
        glib::timeout_add_local_once(std::time::Duration::from_secs(120), || {
            eprintln!("[{APP}] closing automatically after 120s idle");
            gtk::main_quit();
        });
    }

    gtk::main();

    // Release the grab and destroy the window BEFORE returning: the caller is
    // about to run xdotool ctrl+v, and a live keyboard grab would swallow those
    // keys, while a window still on top could receive them itself.
    if let Some(seat) = seat_holder.borrow_mut().take() {
        seat.ungrab();
    }
    unsafe {
        win.destroy();
    }
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }

    let picked = ctx.result.borrow().clone();
    picked
}

/// Try to grab the seat for this window, retrying ~10 times while X maps it.
fn try_grab(win: gtk::Window, holder: Rc<RefCell<Option<gdk::Seat>>>, attempt: u32) {
    let (Some(gw), Some(seat)) = (
        win.window(),
        gdk::Display::default().and_then(|d| d.default_seat()),
    ) else {
        return;
    };
    let status = seat.grab(&gw, gdk::SeatCapabilities::ALL, true, None, None, None);
    if status == gdk::GrabStatus::Success {
        *holder.borrow_mut() = Some(seat);
        return;
    }
    if attempt < 10 {
        glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            try_grab(win, holder, attempt + 1)
        });
    } else {
        eprintln!("[{APP}] could not grab pointer ({:?}); falling back to focus-out", status);
    }
}

fn load_css() {
    let dark = crate::util::setting("theme").as_deref() == Some("dark");
    // Panel background opacity. Only visible while a compositor is running.
    let alpha = crate::util::setting("opacity")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v.clamp(0.30, 1.0))
        .unwrap_or(0.88);
    let base = if dark { CSS_DARK } else { CSS_LIGHT }
        .replace("__ALPHA__", &format!("{:.2}", alpha));
    let base = base.as_str();
    let css = format!(
        "* {{ font-family: {}; }}\n.body {{ font-family: {}; }}\n{}",
        FONT_UI, FONT_MONO, base
    );
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(css.as_bytes()).is_err() {
        return;
    }
    if let Some(screen) = gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn place_near_pointer(win: &gtk::Window) {
    let Some(display) = gdk::Display::default() else { return };
    let Some(seat) = display.default_seat() else { return };
    let Some(pointer) = seat.pointer() else { return };
    let (_s, px, py) = pointer.position();
    let (mut x, mut y) = (px - W / 2, py - 40);
    if let Some(mon) = display.monitor_at_point(px, py) {
        let g = mon.geometry();
        x = x.clamp(g.x() + 8, g.x() + g.width() - W - 8);
        y = y.clamp(g.y() + 8, g.y() + g.height() - H - 8);
    }
    win.move_(x, y);
}

/// How many lines of text each row shows by default.
const PREVIEW_LINES: f64 = 2.0;

/// Inline image preview shown in the row, much smaller than the old card.
fn thumbnail_row(ctx: &Ctx, path: &str) -> Option<Pixbuf> {
    let key = format!("r:{}", path);
    if let Some(p) = ctx.pix.borrow().get(&key) {
        return Some(p.clone());
    }
    let pb = Pixbuf::from_file_at_scale(path, 240, 104, true).ok()?;
    ctx.pix.borrow_mut().insert(key, pb.clone());
    Some(pb)
}

fn row_title(e: &Entry) -> String {
    if e.is_image() {
        let dims = e.meta_text();
        let dims = dims.split(" - ").next().unwrap_or("").trim();
        return if dims.is_empty() {
            "Image".to_string()
        } else {
            format!("Image {}", dims)
        };
    }
    // Flatten the whole content into a single run of words so the wrapped,
    // two-line preview fills both lines for long text - whether the original
    // was one long line or several short ones.
    let flat = e.content.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        "(empty)".to_string()
    } else {
        truncate(&flat, 220)
    }
}

fn build_row(ctx: &Ctx, e: &Entry, _n: usize) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let hb = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    hb.style_context().add_class("rowbox");

    // Entry title, plus an inline preview underneath for image entries
    let vb = gtk::Box::new(gtk::Orientation::Vertical, 2);
    vb.set_valign(gtk::Align::Center);

    let title = gtk::Label::new(Some(&row_title(e)));
    title.set_xalign(0.0);
    title.set_yalign(0.0);
    title.set_max_width_chars(1); // let the label shrink instead of widening the panel
    title.style_context().add_class("rowtitle");

    if e.is_image() {
        // Image rows only carry a short caption, so keep them on one line.
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        vb.pack_start(&title, false, false, 0);
    } else {
        // Wrapping preview capped at PREVIEW_LINES lines. The three settings
        // below must go together: wrap + lines(n) + ellipsize is the only combo
        // GTK renders as "wrap to n lines, then …". Dropping ellipsize (or
        // wrapping the label in a ScrolledWindow) makes it collapse back to a
        // single line - that was the earlier bug. max_width_chars(1) set above
        // keeps the label from widening the panel; it still wraps to the width
        // the row gives it.
        title.set_line_wrap(true);
        title.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
        title.set_lines(PREVIEW_LINES as i32);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        vb.pack_start(&title, true, true, 0);
    }

    if e.is_image() {
        if let Some(pb) = e.path.as_deref().and_then(|p| thumbnail_row(ctx, p)) {
            let img = gtk::Image::from_pixbuf(Some(&pb));
            img.set_halign(gtk::Align::Start);
            img.set_margin_top(4);
            img.style_context().add_class("rowimg");
            vb.pack_start(&img, false, false, 0);
        }
    }
    hb.pack_start(&vb, true, true, 0);

    // Clicking the row already copies, so the trailing button is for pinning.
    let pin = pin_button(e.pinned);
    pin.set_valign(gtk::Align::Center);
    pin.set_tooltip_text(Some(if e.pinned {
        "Unpin (Ctrl+P)"
    } else {
        "Pin this entry (Ctrl+P)"
    }));
    hb.pack_end(&pin, false, false, 0);

    row.add(&hb);

    let id = e.id;
    let pinned = e.pinned;
    {
        let ctx = ctx.clone();
        pin.connect_clicked(move |_| {
            let _ = db::set_pinned(&ctx.con, id, !pinned);
            *ctx.all.borrow_mut() = db::fetch(&ctx.con, 300).unwrap_or_default();
            rebuild(&ctx, 0);
        });
    }
    row
}

/// Pin button. Icon names differ between icon themes, so probe the candidates
/// in order and fall back to the ★ / ☆ glyphs when none of them exist.
fn pin_button(pinned: bool) -> gtk::Button {
    let btn = gtk::Button::new();
    let candidates: &[&str] = if pinned {
        &["view-pin-symbolic", "starred-symbolic", "starred", "bookmark-new"]
    } else {
        &[
            "view-pin-symbolic",
            "non-starred-symbolic",
            "non-starred",
            "bookmark-new",
        ]
    };
    let theme = gtk::IconTheme::default();
    let found = theme.and_then(|t| {
        candidates
            .iter()
            .find(|n| t.has_icon(n))
            .map(|n| n.to_string())
    });
    match found {
        Some(name) => {
            let img = gtk::Image::from_icon_name(Some(&name), gtk::IconSize::Menu);
            img.set_pixel_size(12);
            btn.add(&img)
        }
        None => btn.add(&gtk::Label::new(Some(if pinned { "★" } else { "☆" }))),
    }
    btn.set_relief(gtk::ReliefStyle::None);
    btn.style_context().add_class("circlebtn");
    if pinned {
        btn.style_context().add_class("circlebtn-on");
    }
    btn
}

fn rebuild(ctx: &Ctx, keep: i32) {
    for child in ctx.list.children() {
        unsafe {
            child.destroy();
        }
    }
    let q = ctx.search.text().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();

    let mut shown = Vec::new();
    for e in ctx.all.borrow().iter() {
        let hay = if e.is_image() {
            e.label(999).to_lowercase()
        } else {
            e.content.to_lowercase()
        };
        if terms.iter().all(|t| hay.contains(t)) {
            shown.push(e.clone());
        }
    }
    shown.truncate(RENDER_CAP);

    for (i, e) in shown.iter().enumerate() {
        let row = build_row(ctx, e, i + 1);
        ctx.list.add(&row);
    }
    ctx.list.show_all();

    let total = ctx.all.borrow().len();
    ctx.foot.set_text(&format!(
        "{}/{} items  •  ⏎ copy  •  ⌥1-9 quick pick  •  ⌃P pin  •  ⌦ delete  •  esc close",
        shown.len(),
        total
    ));
    *ctx.shown.borrow_mut() = shown;

    let n = ctx.shown.borrow().len() as i32;
    if n > 0 {
        select_index(ctx, keep.clamp(0, n - 1));
    }
}

fn select_index(ctx: &Ctx, i: i32) {
    if let Some(row) = ctx.list.row_at_index(i) {
        ctx.list.select_row(Some(&row));
        row.grab_focus();
        ctx.search.grab_focus_without_selecting();
    }
}

fn selected(ctx: &Ctx) -> Option<Entry> {
    let i = ctx.list.selected_row()?.index();
    ctx.shown.borrow().get(i as usize).cloned()
}

fn accept(ctx: &Ctx) {
    if let Some(e) = selected(ctx) {
        *ctx.result.borrow_mut() = Some(e);
    }
    gtk::main_quit();
}

fn move_sel(ctx: &Ctx, delta: i32) {
    let n = ctx.shown.borrow().len() as i32;
    if n == 0 {
        return;
    }
    let cur = ctx.list.selected_row().map(|r| r.index()).unwrap_or(0);
    select_index(ctx, (cur + delta).clamp(0, n - 1));
}

fn on_key(ctx: &Ctx, ev: &gdk::EventKey) -> glib::Propagation {
    use gdk::keys::constants as k;
    let key = ev.keyval();
    let state = ev.state();
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let alt = state.contains(gdk::ModifierType::MOD1_MASK);

    // Alt+1..9: quick pick
    if alt {
        if let Some(c) = key.to_unicode() {
            if ('1'..='9').contains(&c) {
                let idx = c as i32 - '1' as i32;
                if idx < ctx.shown.borrow().len() as i32 {
                    select_index(ctx, idx);
                    accept(ctx);
                }
                return glib::Propagation::Stop;
            }
        }
    }

    match key {
        k::Escape => {
            gtk::main_quit();
            glib::Propagation::Stop
        }
        k::Return | k::KP_Enter => {
            accept(ctx);
            glib::Propagation::Stop
        }
        k::Down => {
            move_sel(ctx, 1);
            glib::Propagation::Stop
        }
        k::Up => {
            move_sel(ctx, -1);
            glib::Propagation::Stop
        }
        k::Page_Down => {
            move_sel(ctx, 5);
            glib::Propagation::Stop
        }
        k::Page_Up => {
            move_sel(ctx, -5);
            glib::Propagation::Stop
        }
        k::Delete => {
            if let Some(e) = selected(ctx) {
                let _ = db::delete(&ctx.con, e.id);
                ctx.all.borrow_mut().retain(|x| x.id != e.id);
                let keep = ctx.list.selected_row().map(|r| r.index()).unwrap_or(0);
                rebuild(ctx, keep);
            }
            glib::Propagation::Stop
        }
        k::p if ctrl => {
            if let Some(e) = selected(ctx) {
                let _ = db::set_pinned(&ctx.con, e.id, !e.pinned);
                *ctx.all.borrow_mut() = db::fetch(&ctx.con, 300).unwrap_or_default();
                rebuild(ctx, 0);
            }
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    }
}
