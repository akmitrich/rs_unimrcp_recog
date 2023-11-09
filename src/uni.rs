#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]
#![allow(rustdoc::broken_intra_doc_links)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const FALSE: apt_bool_t = 0;
pub const TRUE: apt_bool_t = 1;

pub unsafe fn inline_mrcp_engine_open_respond(
    engine: *mut mrcp_engine_t,
    status: apt_bool_t,
) -> apt_bool_t {
    (*(*engine).event_vtable).on_open.unwrap()(engine, status)
}

pub unsafe fn inline_mrcp_engine_close_respond(engine: *mut mrcp_engine_t) -> apt_bool_t {
    (*(*engine).event_vtable).on_close.unwrap()(engine)
}

pub unsafe fn inline_mrcp_engine_channel_open_respond(
    channel: *mut mrcp_engine_channel_t,
    status: apt_bool_t,
) -> apt_bool_t {
    (*(*channel).event_vtable).on_open.unwrap()(channel, status)
}

pub unsafe fn inline_mrcp_engine_channel_close_respond(
    channel: *mut mrcp_engine_channel_t,
) -> apt_bool_t {
    (*(*channel).event_vtable).on_close.unwrap()(channel)
}

pub unsafe fn inline_mpf_source_stream_capabilities_create(
    pool: *mut apr_pool_t,
) -> *mut mpf_stream_capabilities_t {
    mpf_stream_capabilities_create(STREAM_DIRECTION_RECEIVE, pool)
}

pub unsafe fn inline_mpf_sink_stream_capabilities_create(
    pool: *mut apr_pool_t,
) -> *mut mpf_stream_capabilities_t {
    mpf_stream_capabilities_create(STREAM_DIRECTION_SEND, pool)
}

pub unsafe fn inline_mpf_codec_capabilities_add(
    capabilities: *mut mpf_codec_capabilities_t,
    sample_rates: std::os::raw::c_int,
    codec_name: *const i8,
) -> apt_bool_t {
    let attribs = apr_array_push((*capabilities).attrib_arr) as *mut mpf_codec_attribs_t;
    inline_apt_string_assign(
        &mut (*attribs).name as _,
        codec_name,
        (*(*capabilities).attrib_arr).pool,
    );
    (*attribs).sample_rates = sample_rates;
    (*attribs).bits_per_sample = 0;
    // (*attribs).frame_duration = CODEC_FRAME_TIME_BASE as _; // In version 1.8.0 was introduced 'frame_duration' codec property. 10 ms was hardcoded in earlier versions
    TRUE
}

pub unsafe fn inline_apt_string_assign(str: *mut apt_str_t, src: *const i8, pool: *mut apr_pool_t) {
    (*str).buf = std::ptr::null_mut() as _;
    (*str).length = if src.is_null() { 0 } else { libc::strlen(src) };
    if (*str).length > 0 {
        (*str).buf = apr_pstrmemdup(pool, src, (*str).length);
    }
}

pub unsafe fn inline_mrcp_generic_header_property_check(
    message: *const mrcp_message_t,
    id: apr_size_t,
) -> apt_bool_t {
    inline_apt_header_section_field_check(&(*message).header.header_section as _, id)
}

pub unsafe fn inline_apt_header_section_field_check(
    header: *const apt_header_section_t,
    id: apr_size_t,
) -> apt_bool_t {
    let arr_size = (*header).arr_size;
    let arr = std::slice::from_raw_parts((*header).arr, arr_size);
    if id < arr_size {
        return if arr[id].is_null() { FALSE } else { TRUE };
    }
    FALSE
}

pub unsafe fn inline_mrcp_generic_header_get(
    message: *const mrcp_message_t,
) -> *mut mrcp_generic_header_t {
    (*message).header.generic_header_accessor.data as _
}

pub unsafe fn inline_mrcp_engine_channel_message_send(
    channel: *mut mrcp_engine_channel_t,
    message: *mut mrcp_message_t,
) -> apt_bool_t {
    (*(*channel).event_vtable).on_message.unwrap()(channel, message)
}

pub unsafe fn inline_mrcp_resource_header_get(message: *const mrcp_message_t) -> *mut libc::c_void {
    (*message).header.resource_header_accessor.data
}

pub unsafe fn inline_mrcp_resource_header_property_check(
    message: *const mrcp_message_t,
    id: apr_size_t,
) -> apt_bool_t {
    inline_apt_header_section_field_check(
        &(*message).header.header_section as _,
        id + GENERIC_HEADER_COUNT as usize,
    )
}

pub unsafe fn inline_mrcp_resource_header_prepare(
    mrcp_message: *mut mrcp_message_t,
) -> *mut libc::c_void {
    inline_mrcp_header_allocate(
        &mut (*mrcp_message).header.resource_header_accessor as _,
        (*mrcp_message).pool,
    )
}

pub unsafe fn inline_mrcp_header_allocate(
    accessor: *mut mrcp_header_accessor_t,
    pool: *mut apr_pool_t,
) -> *mut libc::c_void {
    if !(*accessor).data.is_null() {
        return (*accessor).data;
    }
    if (*accessor).vtable.is_null() || (*(*accessor).vtable).allocate.is_none() {
        return std::ptr::null_mut() as _;
    }
    (*(*accessor).vtable).allocate.unwrap()(accessor, pool)
}

pub unsafe fn inline_apt_string_set(str: *mut apt_str_t, src: *const i8) {
    (*str).buf = src as _;
    (*str).length = if src.is_null() { 0 } else { libc::strlen(src) }
}

pub unsafe fn inline_mrcp_generic_header_prepare(
    message: *mut mrcp_message_t,
) -> *mut mrcp_generic_header_t {
    inline_mrcp_header_allocate(
        &mut (*message).header.generic_header_accessor as _,
        (*message).pool,
    ) as _
}

pub unsafe fn inline_apt_string_assign_n(
    str: *mut apt_str_t,
    src: *const i8,
    length: apr_size_t,
    pool: *mut apr_pool_t,
) {
    (*str).buf = std::ptr::null_mut() as _;
    (*str).length = length;
    if (*str).length > 0 {
        (*str).buf = apr_pstrmemdup(pool, src, (*str).length);
    }
}
