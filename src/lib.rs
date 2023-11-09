#![allow(clippy::missing_safety_doc)]
use std::{io::Write, mem::size_of};

use recog_buffer::RecogBuffer;
use speech_detector::SpeechDetectorEvent;

mod recog_buffer;
mod speech_detector;
pub mod uni;

const RECOG_ENGINE_TASK_NAME: &[u8; 16] = b"Rust ASR-Engine\0";

pub static ENGINE_VTABLE: uni::mrcp_engine_method_vtable_t = uni::mrcp_engine_method_vtable_t {
    destroy: Some(engine_destroy),
    open: Some(engine_open),
    close: Some(engine_close),
    create_channel: Some(engine_create_channel),
};

static CHANNEL_VTABLE: uni::mrcp_engine_channel_method_vtable_t =
    uni::mrcp_engine_channel_method_vtable_t {
        destroy: Some(channel_destroy),
        open: Some(channel_open),
        close: Some(channel_close),
        process_request: Some(channel_process_request),
    };

static STREAM_VTABLE: uni::mpf_audio_stream_vtable_t = uni::mpf_audio_stream_vtable_t {
    destroy: Some(stream_destroy),
    open_rx: None,
    close_rx: None,
    read_frame: None,
    open_tx: Some(stream_open),
    close_tx: Some(stream_close),
    write_frame: Some(stream_write),
    trace: None,
};

#[repr(C)]
struct DemoRecogEngine {
    task: *mut uni::apt_consumer_task_t,
}

#[derive(Debug)]
#[repr(C)]
struct DemoRecogChannel {
    custom_engine: *mut DemoRecogEngine,
    channel: *mut uni::mrcp_engine_channel_t,
    recog_request: *mut uni::mrcp_message_t,
    stop_response: *mut uni::mrcp_message_t,
    audio_buffer: *mut RecogBuffer,
}

#[repr(C)]
enum RecogMsgType {
    OpenChannel,
    CloseChannel,
    RequestProcess,
}

#[repr(C)]
struct RecogMsg {
    type_: RecogMsgType,
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
}

#[no_mangle]
pub static mut mrcp_plugin_version: uni::mrcp_plugin_version_t = uni::mrcp_plugin_version_t {
    major: uni::PLUGIN_MAJOR_VERSION as i32,
    minor: uni::PLUGIN_MINOR_VERSION as i32,
    patch: uni::PLUGIN_PATCH_VERSION as i32,
    is_dev: 0,
};

#[no_mangle]
pub unsafe extern "C" fn mrcp_plugin_create(pool: *mut uni::apr_pool_t) -> *mut uni::mrcp_engine_t {
    env_logger::init();
    log::debug!(
        "[DEMO_RECOG] Going to Create ASR-Engine on pool = {:?}",
        pool
    );

    let custom_engine = uni::apr_palloc(pool, size_of::<DemoRecogEngine>()) as *mut DemoRecogEngine;
    let msg_pool = uni::apt_task_msg_pool_create_dynamic(size_of::<RecogMsg>(), pool);
    (*custom_engine).task = uni::apt_consumer_task_create(custom_engine as _, msg_pool, pool);
    if (*custom_engine).task.is_null() {
        return std::ptr::null_mut();
    }
    let task = uni::apt_consumer_task_base_get((*custom_engine).task);
    uni::apt_task_name_set(task, RECOG_ENGINE_TASK_NAME.as_ptr() as _);
    let vtable = uni::apt_task_vtable_get(task);
    if !vtable.is_null() {
        (*vtable).process_msg = Some(demo_recog_msg_process);
    }
    let engine = uni::mrcp_engine_create(
        uni::MRCP_RECOGNIZER_RESOURCE as _,
        custom_engine as _,
        &ENGINE_VTABLE as _,
        pool,
    );
    log::debug!("[DEMO_RECOG] ASR-Engine Created: {:?}", engine);
    engine
}

unsafe extern "C" fn engine_destroy(engine: *mut uni::mrcp_engine_t) -> uni::apt_bool_t {
    let custom_engine = (*engine).obj as *mut DemoRecogEngine;
    log::debug!(
        "[DEMO_RECOG] Destroy Engine {:?}. Custom engine = {:?}",
        engine,
        custom_engine
    );
    if !(*custom_engine).task.is_null() {
        let task = uni::apt_consumer_task_base_get((*custom_engine).task);
        let destroyed = uni::apt_task_destroy(task);
        (*custom_engine).task = std::ptr::null_mut() as _;
        log::debug!("[DEMO_RECOG] Task {:?} destroyed = {:?}", task, destroyed);
    }
    uni::TRUE
}

unsafe extern "C" fn engine_open(engine: *mut uni::mrcp_engine_t) -> uni::apt_bool_t {
    let custom_engine = (*engine).obj as *mut DemoRecogEngine;
    log::debug!(
        "[DEMO_RECOG] Open Engine {:?}. Custom engine = {:?}",
        engine,
        custom_engine
    );
    if !(*custom_engine).task.is_null() {
        let task = uni::apt_consumer_task_base_get((*custom_engine).task);
        let started = uni::apt_task_start(task);
        log::debug!("[DEMO_RECOG] Task = {:?} started = {:?}.", task, started);
    }
    log::debug!("[DEMO_RECOG] Opened with Safe Engine: {:?}", custom_engine);
    uni::inline_mrcp_engine_open_respond(engine, uni::TRUE)
}

unsafe extern "C" fn engine_close(engine: *mut uni::mrcp_engine_t) -> uni::apt_bool_t {
    let custom_engine = (*engine).obj as *mut DemoRecogEngine;
    log::debug!(
        "[DEMO_RECOG] Close Engine {:?}. Custom engine = {:?}",
        engine,
        custom_engine
    );
    if !(*custom_engine).task.is_null() {
        let task = uni::apt_consumer_task_base_get((*custom_engine).task);
        let terminated = uni::apt_task_terminate(task, uni::TRUE);
        log::debug!(
            "[DEMO_RECOG] Task = {:?} terminated = {:?}.",
            task,
            terminated
        );
    }
    uni::inline_mrcp_engine_close_respond(engine)
}

unsafe extern "C" fn engine_create_channel(
    engine: *mut uni::mrcp_engine_t,
    pool: *mut uni::apr_pool_t,
) -> *mut uni::mrcp_engine_channel_t {
    log::debug!(
        "[DEMO_RECOG] Engine {:?} is going to create a channel",
        engine
    );

    let demo_channel =
        uni::apr_palloc(pool, size_of::<DemoRecogChannel>()) as *mut DemoRecogChannel;
    (*demo_channel).custom_engine = (*engine).obj as _;
    (*demo_channel).recog_request = std::ptr::null_mut() as _;
    (*demo_channel).stop_response = std::ptr::null_mut() as _;
    (*demo_channel).audio_buffer = RecogBuffer::leaked();

    let capabilities = uni::inline_mpf_sink_stream_capabilities_create(pool);
    uni::inline_mpf_codec_capabilities_add(
        &mut (*capabilities).codecs as _,
        uni::MPF_SAMPLE_RATE_8000 as _,
        b"LPCM\0".as_ptr() as _,
    );

    let termination = uni::mrcp_engine_audio_termination_create(
        demo_channel as _,
        &STREAM_VTABLE as _,
        capabilities,
        pool,
    );
    (*demo_channel).channel = uni::mrcp_engine_channel_create(
        engine,
        &CHANNEL_VTABLE as _,
        demo_channel as _,
        termination,
        pool,
    );
    log::debug!(
        "[DEMO_RECOG] Engine created channel = {:?}",
        (*demo_channel).channel,
    );
    (*demo_channel).channel
}

pub unsafe extern "C" fn channel_destroy(
    channel: *mut uni::mrcp_engine_channel_t,
) -> uni::apt_bool_t {
    log::debug!("[DEMO_RECOG] Channel {:?} destroy.", channel);
    let demo_channel = (*channel).method_obj as *mut DemoRecogChannel;
    RecogBuffer::destroy((*demo_channel).audio_buffer);
    uni::TRUE
}

pub unsafe extern "C" fn channel_open(channel: *mut uni::mrcp_engine_channel_t) -> uni::apt_bool_t {
    log::debug!("[DEMO_RECOG] Channel {:?} open.", channel);
    if !(*channel).attribs.is_null() {
        let header = uni::apr_table_elts((*channel).attribs);
        let entry = (*header).elts as *mut uni::apr_table_entry_t;
        for i in 0..(*header).nelts {
            let entry = entry.offset(i as _);
            let key = std::ffi::CStr::from_ptr((*entry).key);
            let val = std::ffi::CStr::from_ptr((*entry).val);
            log::info!("Attrib name {:?} value {:?}", key, val);
        }
    }
    demo_recog_msg_signal(
        RecogMsgType::OpenChannel,
        channel,
        std::ptr::null_mut() as _,
    )
}

unsafe extern "C" fn channel_close(channel: *mut uni::mrcp_engine_channel_t) -> uni::apt_bool_t {
    log::debug!("[DEMO_RECOG] Channel {:?} close.", channel);
    demo_recog_msg_signal(
        RecogMsgType::CloseChannel,
        channel,
        std::ptr::null_mut() as _,
    )
}

unsafe extern "C" fn channel_process_request(
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    log::debug!(
        "[DEMO_RECOG] Channel {:?} process request {:?}.",
        channel,
        (*request).start_line.method_id
    );
    demo_recog_msg_signal(RecogMsgType::RequestProcess, channel, request)
}

unsafe fn demo_recog_channel_recognize(
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
    response: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let demo_channel = (*channel).method_obj as *mut DemoRecogChannel;
    let descriptor = uni::mrcp_engine_sink_stream_codec_get(channel);

    if descriptor.is_null() {
        log::error!("Failed to Get Codec Descriptor from channel {:?}", channel);
        (*response).start_line.status_code = uni::MRCP_STATUS_CODE_METHOD_FAILED;
        return uni::FALSE;
    }
    (*(*demo_channel).audio_buffer).prepare(request);

    (*response).start_line.request_state = uni::MRCP_REQUEST_STATE_INPROGRESS;
    uni::inline_mrcp_engine_channel_message_send(channel, response);

    (*demo_channel).recog_request = request;
    uni::TRUE
}

unsafe fn demo_recog_channel_stop(
    channel: *mut uni::mrcp_engine_channel_t,
    _request: *mut uni::mrcp_message_t,
    response: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let demo_channel = (*channel).method_obj as *mut DemoRecogChannel;
    (*demo_channel).stop_response = response;
    uni::TRUE
}

unsafe fn demo_recog_channel_timers_start(
    channel: *mut uni::mrcp_engine_channel_t,
    _request: *mut uni::mrcp_message_t,
    response: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let demo_channel = (*channel).method_obj as *mut DemoRecogChannel;
    (*(*demo_channel).audio_buffer).start_input_timers();
    uni::inline_mrcp_engine_channel_message_send(channel, response)
}

unsafe fn demo_recog_channel_request_dispatch(
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let mut processed = uni::FALSE;
    let response = uni::mrcp_response_create(request, (*request).pool);
    match (*request).start_line.method_id as u32 {
        uni::RECOGNIZER_SET_PARAMS => {}
        uni::RECOGNIZER_GET_PARAMS => {}
        uni::RECOGNIZER_DEFINE_GRAMMAR => {}
        uni::RECOGNIZER_RECOGNIZE => {
            processed = demo_recog_channel_recognize(channel, request, response);
        }
        uni::RECOGNIZER_GET_RESULT => {}
        uni::RECOGNIZER_START_INPUT_TIMERS => {
            processed = demo_recog_channel_timers_start(channel, request, response);
        }
        uni::RECOGNIZER_STOP => {
            processed = demo_recog_channel_stop(channel, request, response);
        }
        x => {
            log::error!("Unexpected method id={}", x);
        }
    }
    if processed == uni::FALSE {
        uni::inline_mrcp_engine_channel_message_send(channel, response);
    }
    uni::TRUE
}

pub unsafe extern "C" fn stream_destroy(_stream: *mut uni::mpf_audio_stream_t) -> uni::apt_bool_t {
    uni::TRUE
}

pub unsafe extern "C" fn stream_open(
    _stream: *mut uni::mpf_audio_stream_t,
    _codec: *mut uni::mpf_codec_t,
) -> uni::apt_bool_t {
    uni::TRUE
}

pub unsafe extern "C" fn stream_close(_stream: *mut uni::mpf_audio_stream_t) -> uni::apt_bool_t {
    uni::TRUE
}

unsafe fn demo_recog_start_of_input(recog_channel: *mut DemoRecogChannel) -> uni::apt_bool_t {
    let message = uni::mrcp_event_create(
        (*recog_channel).recog_request,
        uni::RECOGNIZER_START_OF_INPUT as _,
        (*(*recog_channel).recog_request).pool,
    );
    if message.is_null() {
        log::error!("Unable to create event START OF INPUT");
        return uni::FALSE;
    }
    (*message).start_line.request_state = uni::MRCP_REQUEST_STATE_INPROGRESS;
    uni::inline_mrcp_engine_channel_message_send((*recog_channel).channel, message)
}

unsafe fn demo_recog_result_load(
    recognized: &str,
    message: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let generic_header = uni::inline_mrcp_generic_header_prepare(message);
    if !generic_header.is_null() {
        uni::inline_apt_string_assign(
            &mut (*generic_header).content_type as _,
            b"text/plain; charset=UTF-8\0".as_ptr() as _,
            (*message).pool,
        );
        uni::mrcp_generic_header_property_add(message, uni::GENERIC_HEADER_CONTENT_TYPE as _);
    }
    let result = recognized.as_bytes();
    uni::inline_apt_string_assign_n(
        &mut (*message).body as _,
        result.as_ptr() as _,
        result.len(),
        (*message).pool,
    );

    uni::TRUE
}

unsafe fn demo_recog_recognition_process(
    recog_channel: *mut DemoRecogChannel,
    recog_event: SpeechDetectorEvent,
) -> uni::apt_bool_t {
    let mut recognized = String::new();
    log::info!("Event: {:?}", recog_event);
    let cause = match recog_event {
        SpeechDetectorEvent::None => return uni::FALSE,
        SpeechDetectorEvent::Activity => {
            log::trace!("Detected Voice Activity in {:?}", (*recog_channel).channel);
            if !(*(*recog_channel).audio_buffer).input_started() {
                (*(*recog_channel).audio_buffer).start_input();
                return demo_recog_start_of_input(recog_channel);
            } else {
                return uni::TRUE;
            }
        }
        SpeechDetectorEvent::Inactivity { duration } => {
            log::trace!(
                "Detected Voice {:?} in {:?}",
                recog_event,
                (*recog_channel).channel
            );
            (*(*recog_channel).audio_buffer).recognize(duration);
            return uni::TRUE;
        }
        SpeechDetectorEvent::DurationTimeout => {
            log::trace!(
                "Detected Duration Timeout in {:?}",
                (*recog_channel).channel
            );
            (*(*recog_channel).audio_buffer)
                .recognize((*(*recog_channel).audio_buffer).duration_timeout());
            return uni::TRUE;
        }
        SpeechDetectorEvent::Noinput => {
            log::error!("Detected Noinput. Channel {:?}", (*recog_channel).channel);
            uni::RECOGNIZER_COMPLETION_CAUSE_NO_INPUT_TIMEOUT
        }
        SpeechDetectorEvent::Recognizing => match (*(*recog_channel).audio_buffer).load_result() {
            None => return uni::FALSE,
            Some(result) => {
                recognized = result;
                uni::RECOGNIZER_COMPLETION_CAUSE_SUCCESS
            }
        },
    };
    let message = uni::mrcp_event_create(
        (*recog_channel).recog_request,
        uni::RECOGNIZER_RECOGNITION_COMPLETE as _,
        (*(*recog_channel).recog_request).pool,
    );
    if message.is_null() {
        log::error!("Unable to create event RECOGNITION COMPLETE");
        return uni::FALSE;
    }
    let recog_header =
        uni::inline_mrcp_resource_header_prepare(message) as *mut uni::mrcp_recog_header_t;
    if !recog_header.is_null() {
        (*recog_header).completion_cause = cause;
        uni::mrcp_resource_header_property_add(
            message,
            uni::RECOGNIZER_HEADER_COMPLETION_CAUSE as _,
        );
    }
    (*message).start_line.request_state = uni::MRCP_REQUEST_STATE_COMPLETE;
    if cause == uni::RECOGNIZER_COMPLETION_CAUSE_SUCCESS {
        if recognized.is_empty() {
            (*(*recog_channel).audio_buffer).restart_writing();
            return uni::FALSE;
        }
        demo_recog_result_load(recognized.as_str(), message);
        log::info!(
            "[DEMO_RECOG] Load for {:?}: {:?} ({} bytes)",
            (*recog_channel).channel,
            recognized,
            recognized.as_bytes().len()
        );
    }
    (*recog_channel).recog_request = std::ptr::null_mut() as _;
    uni::inline_mrcp_engine_channel_message_send((*recog_channel).channel, message)
}

pub unsafe extern "C" fn stream_write(
    stream: *mut uni::mpf_audio_stream_t,
    frame: *const uni::mpf_frame_t,
) -> uni::apt_bool_t {
    let demo_channel = (*stream).obj as *mut DemoRecogChannel;
    if !(*demo_channel).stop_response.is_null() {
        uni::inline_mrcp_engine_channel_message_send(
            (*demo_channel).channel,
            (*demo_channel).stop_response,
        );
        (*demo_channel).stop_response = std::ptr::null_mut() as _;
        (*demo_channel).recog_request = std::ptr::null_mut() as _;
        return uni::TRUE;
    }
    if !(*demo_channel).recog_request.is_null() {
        if ((*frame).type_ & (uni::MEDIA_FRAME_TYPE_EVENT as i32))
            == uni::MEDIA_FRAME_TYPE_EVENT as i32
        {
            if (*frame).marker == uni::MPF_MARKER_START_OF_EVENT as i32 {
                log::info!(
                    "Detected Start of Event id: {}",
                    (*frame).event_frame.event_id()
                );
            } else if (*frame).marker == uni::MPF_MARKER_END_OF_EVENT as i32 {
                log::info!(
                    "Detected End of Event id: {}, duration: {}",
                    (*frame).event_frame.event_id(),
                    (*frame).event_frame.duration()
                )
            }
        } else {
            let buf = std::slice::from_raw_parts(
                (*frame).codec_frame.buffer as *mut u8,
                (*frame).codec_frame.size,
            );
            (*(*demo_channel).audio_buffer).write(buf).ok();
            let event = (*(*demo_channel).audio_buffer).detector_event();
            demo_recog_recognition_process(demo_channel, event);
        }
    }
    uni::TRUE
}

unsafe extern "C" fn demo_recog_msg_signal(
    type_: RecogMsgType,
    channel: *mut uni::mrcp_engine_channel_t,
    request: *mut uni::mrcp_message_t,
) -> uni::apt_bool_t {
    let mut status = uni::FALSE;
    let demo_channel = (*channel).method_obj as *mut DemoRecogChannel;
    let demo_engine = (*demo_channel).custom_engine;
    let task = uni::apt_consumer_task_base_get((*demo_engine).task);
    let msg = uni::apt_task_msg_get(task);
    if !msg.is_null() {
        (*msg).type_ = uni::TASK_MSG_USER as _;
        let demo_msg = (*msg).data.as_mut_ptr() as *mut RecogMsg;
        (*demo_msg).type_ = type_;
        (*demo_msg).channel = channel;
        (*demo_msg).request = request;
        status = uni::apt_task_msg_signal(task, msg);
    }
    status
}

unsafe extern "C" fn demo_recog_msg_process(
    _task: *mut uni::apt_task_t,
    msg: *mut uni::apt_task_msg_t,
) -> uni::apt_bool_t {
    let demo_msg = (*msg).data.as_mut_ptr() as *mut RecogMsg;
    match (*demo_msg).type_ {
        RecogMsgType::OpenChannel => {
            uni::inline_mrcp_engine_channel_open_respond((*demo_msg).channel, uni::TRUE);
        }
        RecogMsgType::CloseChannel => {
            uni::inline_mrcp_engine_channel_close_respond((*demo_msg).channel);
        }
        RecogMsgType::RequestProcess => {
            demo_recog_channel_request_dispatch((*demo_msg).channel, (*demo_msg).request);
        }
    }
    uni::TRUE
}
