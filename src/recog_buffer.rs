use std::io::Write;

use crate::{speech_detector::SpeechDetectorEvent, uni};

pub struct RecogBuffer {
    count: usize,
    speech_event: SpeechDetectorEvent,
}

impl RecogBuffer {
    pub fn leaked() -> *mut Self {
        Box::into_raw(Box::new(Self {
            count: 0,
            speech_event: SpeechDetectorEvent::None,
        }))
    }

    pub unsafe fn destroy(this: *mut Self) {
        drop(Box::from_raw(this));
    }

    pub fn prepare(&self, _request: *mut uni::mrcp_message_t) {}

    pub fn detector_event(&self) -> SpeechDetectorEvent {
        self.speech_event
    }

    pub fn start_input_timers(&mut self) {}

    pub fn input_started(&self) -> bool {
        false
    }

    pub fn start_input(&mut self) {}

    pub fn recognize(&mut self, duration: usize) {
        log::info!("Recognizing {} ms", duration);
        self.speech_event = SpeechDetectorEvent::Recognizing;
    }

    pub fn duration_timeout(&self) -> usize {
        20000
    }

    pub fn load_result(&self) -> Option<String> {
        log::info!("Result count: {}", self.count);
        if self.detector_event() == SpeechDetectorEvent::Recognizing {
            Some("Привет, мир!".to_owned())
        } else {
            None
        }
    }

    pub fn restart_writing(&mut self) {
        self.count = 0;
    }
}

impl Write for RecogBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.detector_event() == SpeechDetectorEvent::Recognizing {
            return Ok(0);
        }
        self.count += 1;
        log::debug!("WRITE: {} frames", self.count);
        if self.count < 1100 {
            Ok(buf.len())
        } else {
            self.speech_event = SpeechDetectorEvent::DurationTimeout;
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
