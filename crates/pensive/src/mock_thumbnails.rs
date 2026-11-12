//! Fills the mock browser's thumbnails on a background thread, from the PNGs `cargo mocks` wrote
//! when they're newer than the app, and by rendering each mock otherwise.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, SystemTime},
};

use gpui::{App, RenderImage};
use image::{Frame, RgbaImage, imageops};

/// Wide enough for a card in a two-column grid, at twice its size.
const WIDTH: u32 = 480;

pub fn start(cx: &mut App) {
    let (sender, receiver) = mpsc::channel::<(String, Arc<RenderImage>)>();
    thread::spawn(move || {
        let rendered = Rendered::find();
        for mock in ui::every_mock() {
            let id = mock.id();
            let image = match rendered.as_ref().and_then(|rendered| rendered.read(&id)) {
                Some(image) => image,
                None => match mock.render() {
                    Ok(Some(image)) => image,
                    Ok(None) => return,
                    Err(error) => {
                        eprintln!("couldn't render the thumbnail of {id}: {error:#}");
                        continue;
                    }
                },
            };
            if sender.send((id, thumbnail(image))).is_err() {
                return;
            }
        }
    });
    cx.spawn(async move |cx| {
        loop {
            loop {
                match receiver.try_recv() {
                    Ok((id, thumbnail)) => {
                        cx.update(|cx| ui::MockThumbnails::insert(id, thumbnail, cx))
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
        }
    })
    .detach();
}

/// The `target/mocks` folder next to the app's binary, and when the binary was built.
struct Rendered {
    folder: PathBuf,
    built: SystemTime,
}

impl Rendered {
    fn find() -> Option<Self> {
        let app = std::env::current_exe().ok()?;
        let built = app.metadata().ok()?.modified().ok()?;
        let folder = app.parent()?.parent()?.join("mocks");
        Some(Self { folder, built })
    }

    /// The mock's PNG, unless it's older than the app and may show an older version.
    fn read(&self, id: &str) -> Option<RgbaImage> {
        let path = self.folder.join(format!("{id}.png"));
        let modified = path.metadata().ok()?.modified().ok()?;
        if modified < self.built {
            return None;
        }
        Some(image::open(path).ok()?.into_rgba8())
    }
}

/// Scaled down to `WIDTH`, in the BGRA order GPUI draws.
fn thumbnail(image: RgbaImage) -> Arc<RenderImage> {
    let height = image.height() * WIDTH / image.width().max(1);
    let mut thumbnail = imageops::thumbnail(&image, WIDTH, height);
    for pixel in thumbnail.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![Frame::new(thumbnail)]))
}
