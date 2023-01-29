#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

extern "C" {
    pub fn x264_encoder_open_any(arg1: *mut x264_param_t) -> *mut x264_t;
}
