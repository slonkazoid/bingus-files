use axum::{
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use owo_colors::Style;
use rand::Rng;

pub struct Slonkable<T: serde::Serialize>(T);
impl<T: serde::Serialize> From<T> for Slonkable<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}
impl<T: serde::Serialize> IntoResponse for Slonkable<T> {
    fn into_response(self) -> Response {
        serde_json::to_string(&self.0)
            .map_err(|x| x.to_string())
            .into_response()
    }
}

pub fn sanitize_file_name(name: &str) -> String {
    name.replace(
        ['/', '\\', '&', '?', '"', '\'', '*', '~', '|', ':', '<', '>'],
        "_",
    )
}

pub fn get_random_prefix(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn get_ip(headers: &HeaderMap) -> Option<String> {
    unsafe {
        if headers.contains_key("x-forwarded-for")
            && let Ok(value) = headers.get("x-forwarded-for").unwrap_unchecked().to_str()
        {
            Some(value.split(',').next().unwrap_unchecked().to_string())
        } else {
            None
        }
    }
}

pub fn color_status_code(status_code: u16) -> Style {
    match status_code {
        100..=199 => Style::new().white(),
        200..=299 => Style::new().bright_green(),
        300..=399 => Style::new().yellow(),
        400..=499 => Style::new().bright_red(),
        500..=599 => Style::new().red(),
        _ => Style::new(),
    }
}
