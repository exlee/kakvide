use std::cell::RefCell;
use std::ffi::CString;
use std::path::PathBuf;

use anyhow::{Context, Result};
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::NSApplication;
use objc2_foundation::{NSArray, NSString};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;

thread_local! {
    static OPEN_FILE_PROXY: RefCell<Option<EventLoopProxy<AppEvent>>> = const { RefCell::new(None) };
}

pub fn register_open_file_handler(proxy: EventLoopProxy<AppEvent>) -> Result<()> {
    OPEN_FILE_PROXY.with(|slot| {
        *slot.borrow_mut() = Some(proxy);
    });

    let mtm = MainThreadMarker::new().context("open file handler must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let delegate = app.delegate().context("missing NSApplication delegate")?;
    let delegate_class = AnyObject::class(delegate.as_ref());
    let class_name = CString::new("KakvideApplicationDelegate")?;

    let class = if let Some(mut builder) = ClassBuilder::new(&class_name, delegate_class) {
        unsafe {
            builder.add_method(
                sel!(application:openFiles:),
                handle_open_files as unsafe extern "C-unwind" fn(_, _, _, _),
            );
        }
        builder.register()
    } else {
        AnyClass::get(&class_name).context("failed to register KakvideApplicationDelegate")?
    };

    unsafe {
        AnyObject::set_class(delegate.as_ref(), class);
    }

    Ok(())
}

unsafe extern "C-unwind" fn handle_open_files(
    _this: &mut AnyObject,
    _sel: Sel,
    _sender: &AnyObject,
    filenames: &NSArray<NSString>,
) {
    let paths = filenames_to_paths(filenames);
    if paths.is_empty() {
        return;
    }

    OPEN_FILE_PROXY.with(|slot| {
        if let Some(proxy) = slot.borrow().as_ref() {
            let _ = proxy.send_event(AppEvent::OpenFiles(paths));
        }
    });
}

fn filenames_to_paths(filenames: &NSArray<NSString>) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(filenames.count());
    for index in 0..filenames.count() {
        let filename = unsafe { filenames.objectAtIndex_unchecked(index) };
        let path = autoreleasepool(|pool| unsafe { filename.to_str(pool).to_owned() });
        paths.push(PathBuf::from(path));
    }
    paths
}
