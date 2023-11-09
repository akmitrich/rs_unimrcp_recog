#![allow(dead_code)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SpeechDetectorEvent {
    None,
    Activity,
    Inactivity { duration: usize },
    Noinput,
    DurationTimeout,
    Recognizing,
}
