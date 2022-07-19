use cairo::glib::translate::{from_glib_full, FromGlibPtrFull};
use pangocairo::pango::{self, ffi::PangoAttribute, Attribute};



/*
pub struct AttrScale(*mut PangoAttribute);

impl AttrScale {
    pub fn new(scale_factor: f64) -> Self {
        unsafe { from_glib_full(pango::ffi::pango_attr_scale_new(scale_factor)) }
    }
}

impl FromGlibPtrFull<*mut PangoAttribute> for AttrScale {
    unsafe fn from_glib_full(ptr: *mut PangoAttribute) -> Self {
        assert!(!ptr.is_null());
        Self(ptr)
    }
}
impl FromGlibPtrFull<*mut AttrScale> for AttrScale {
    unsafe fn from_glib_full(ptr: *mut AttrScale) -> Self {
        assert!(!ptr.is_null());
        Self(ptr as *mut _ as *mut PangoAttribute)
    }
}

impl Into<Attribute> for AttrScale {
    fn into(self) -> Attribute {
        unsafe { Attribute::from_glib_full(self) }
    }
}

*/

pub trait AttrScale {
    fn new_scale(scale_factor: f64) -> Self
        where Self: FromGlibPtrFull<*mut PangoAttribute>
    {
        unsafe { from_glib_full(pango::ffi::pango_attr_scale_new(scale_factor)) }
    }
}

impl AttrScale for Attribute {}