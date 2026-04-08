mod kwallet;
pub mod bridge;

extern "C" {
    fn qt_plugin_instance() -> *mut ::std::ffi::c_void;
}

#[used]
#[no_mangle]
static _QT_PLUGIN_INSTANCE: unsafe extern "C" fn() -> *mut ::std::ffi::c_void =
    qt_plugin_instance;
