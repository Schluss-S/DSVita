pub use self::platform::*;
use crate::core::graphics::gpu::{DISPLAY_HEIGHT, DISPLAY_WIDTH};
use crate::core::graphics::gpu_renderer::{ScreenTopology};
use crate::settings::{ScreenMode};

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;

#[cfg(target_os = "vita")]
#[path = "vita.rs"]
mod platform;

pub const PRESENTER_SCREEN_WIDTH: u32 = 960;
pub const PRESENTER_SCREEN_HEIGHT: u32 = 544;

pub enum PresentEvent {
    Inputs { keymap: u32, touch: Option<(u8, u8)> },
    Quit,
}

pub const PRESENTER_AUDIO_SAMPLE_RATE: usize = 48000;
pub const PRESENTER_AUDIO_BUF_SIZE: usize = 1024;

pub struct PresenterScreen {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PresenterScreen {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        PresenterScreen { x, y, width, height }
    }

    const fn is_within(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    const fn normalize(&self, x: u32, y: u32) -> (u32, u32) {
        (x - self.x, y - self.y)
    }
}

pub const PRESENTER_SUB_SCREEN_WIDTH: u32 = PRESENTER_SCREEN_WIDTH / 2;
pub const PRESENTER_SUB_SCREEN_HEIGHT: u32 = DISPLAY_HEIGHT as u32 * PRESENTER_SUB_SCREEN_WIDTH / DISPLAY_WIDTH as u32;
pub const PRESENTER_SUB_TOP_SCREEN: PresenterScreen = PresenterScreen::new(0, (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_SCREEN_HEIGHT) / 2, PRESENTER_SUB_SCREEN_WIDTH, PRESENTER_SUB_SCREEN_HEIGHT);
pub const PRESENTER_SUB_BOTTOM_SCREEN: PresenterScreen = PresenterScreen::new(
    PRESENTER_SUB_SCREEN_WIDTH,
    (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_SCREEN_HEIGHT) / 2,
    PRESENTER_SUB_SCREEN_WIDTH,
    PRESENTER_SUB_SCREEN_HEIGHT,
);

pub const PRESENTER_SUB_ROTATED_SCREEN_HEIGHT: u32 = DISPLAY_WIDTH as u32 * (PRESENTER_SCREEN_HEIGHT / DISPLAY_WIDTH as u32);
pub const PRESENTER_SUB_ROTATED_SCREEN_WIDTH: u32 = DISPLAY_HEIGHT as u32 * PRESENTER_SUB_ROTATED_SCREEN_HEIGHT / DISPLAY_WIDTH as u32;
pub const PRESENTER_SUB_ROTATED_TOP_SCREEN: PresenterScreen = PresenterScreen::new((PRESENTER_SCREEN_WIDTH - 2 * PRESENTER_SUB_ROTATED_SCREEN_WIDTH) / 2, (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_ROTATED_SCREEN_HEIGHT) / 2, PRESENTER_SUB_ROTATED_SCREEN_WIDTH, PRESENTER_SUB_ROTATED_SCREEN_HEIGHT);
pub const PRESENTER_SUB_ROTATED_BOTTOM_SCREEN: PresenterScreen = PresenterScreen::new(
    PRESENTER_SCREEN_WIDTH / 2,
    (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_ROTATED_SCREEN_HEIGHT) / 2,
    PRESENTER_SUB_ROTATED_SCREEN_WIDTH,
    PRESENTER_SUB_ROTATED_SCREEN_HEIGHT,
);

pub const PRESENTER_SUB_RESIZED_SCREEN_WIDTH_TOP: u32 = DISPLAY_WIDTH as u32;
pub const PRESENTER_SUB_RESIZED_SCREEN_WIDTH_BOT: u32 = DISPLAY_WIDTH as u32 * 2;
pub const PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_TOP: u32 = DISPLAY_HEIGHT as u32;
pub const PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_BOT: u32 = DISPLAY_HEIGHT as u32 * 2;
pub const PRESENTER_SUB_RESIZED_TOP_SCREEN: PresenterScreen = PresenterScreen::new(
    (PRESENTER_SCREEN_WIDTH - PRESENTER_SUB_RESIZED_SCREEN_WIDTH_TOP - PRESENTER_SUB_RESIZED_SCREEN_WIDTH_BOT) / 2,
    (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_TOP) / 2,
    PRESENTER_SUB_RESIZED_SCREEN_WIDTH_TOP,
    PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_TOP);
pub const PRESENTER_SUB_RESIZED_BOTTOM_SCREEN: PresenterScreen = PresenterScreen::new(
    PRESENTER_SUB_RESIZED_TOP_SCREEN.x + PRESENTER_SUB_RESIZED_TOP_SCREEN.width,
    (PRESENTER_SCREEN_HEIGHT - PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_BOT) / 2,
    PRESENTER_SUB_RESIZED_SCREEN_WIDTH_BOT,
    PRESENTER_SUB_RESIZED_SCREEN_HEIGHT_BOT,
);

pub const PRESENTER_SUB_REGULAR: ScreenTopology = ScreenTopology {
    top: PRESENTER_SUB_TOP_SCREEN,
    bottom: PRESENTER_SUB_BOTTOM_SCREEN,
    mode: ScreenMode::Regular,
};

pub const PRESENTER_SUB_ROTATED: ScreenTopology = ScreenTopology {
    top: PRESENTER_SUB_ROTATED_TOP_SCREEN,
    bottom: PRESENTER_SUB_ROTATED_BOTTOM_SCREEN,
    mode: ScreenMode::Rotated,
};

pub const PRESENTER_SUB_RESIZED: ScreenTopology = ScreenTopology {
    top: PRESENTER_SUB_RESIZED_TOP_SCREEN,
    bottom: PRESENTER_SUB_RESIZED_BOTTOM_SCREEN,
    mode: ScreenMode::Resized,
};