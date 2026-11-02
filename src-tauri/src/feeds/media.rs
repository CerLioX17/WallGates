//! "Now playing" from any app that integrates with Windows media controls
//! (Spotify, browsers, media players) — no per-app setup required.

use base64::Engine;
use serde_json::{json, Value};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

#[derive(Default)]
pub struct MediaFeed {
    manager: Option<Manager>,
    last_track: String,
    thumbnail: Option<String>,
}

impl MediaFeed {
    pub fn sample(&mut self) -> Option<Value> {
        if self.manager.is_none() {
            self.manager = Manager::RequestAsync().ok()?.get().ok();
        }
        let manager = self.manager.as_ref()?;
        let Ok(session) = manager.GetCurrentSession() else {
            return Some(json!({ "title": "", "artist": "", "album": "", "playing": false, "thumbnail": null }));
        };
        let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
        let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
        let playing = session
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|s| s == Status::Playing)
            .unwrap_or(false);

        let track = format!("{title}\u{1}{artist}\u{1}{album}");
        if track != self.last_track {
            self.last_track = track;
            self.thumbnail = read_thumbnail(&props).ok();
        }
        Some(json!({
            "title": title, "artist": artist, "album": album,
            "playing": playing, "thumbnail": self.thumbnail,
        }))
    }
}

fn read_thumbnail(
    props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> windows::core::Result<String> {
    let reference = props.Thumbnail()?;
    let stream = reference.OpenReadAsync()?.get()?;
    let size = stream.Size()?.min(4 * 1024 * 1024) as u32;
    let content_type = stream.ContentType().map(|s| s.to_string()).unwrap_or_else(|_| "image/png".into());
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.get()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    let ct = if content_type.starts_with("image/") { content_type } else { "image/png".into() };
    Ok(format!("data:{ct};base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}
