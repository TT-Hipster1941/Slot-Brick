use std::path::Path;

use slot_gfx::WHOLE_TEXTURE;
#[cfg(all(test, feature = "brick"))]
use slot_gfx::{SRC_H, SRC_W};
use slot_store::{ini, Platform};

pub const VIDEO_MODE_FILE: &str = if cfg!(feature = "brick") {
    "Config/brick_video_mode_v3.ini"
} else {
    "Config/video_mode.ini"
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoMode {
    Actual,
    Stretch,
    /// Game Boy / Game Boy Color on the Brick only: the largest whole multiple of the picture
    /// that fits the panel, black around it, so every game pixel is the same size on screen.
    Integer,
}

impl Default for VideoMode {
    fn default() -> Self { Self::Actual }
}

impl VideoMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VideoMode::Actual => "actual",
            VideoMode::Stretch => "stretch",
            VideoMode::Integer => "integer",
        }
    }

    pub fn parse(s: &str) -> Option<VideoMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "actual" => Some(VideoMode::Actual),
            "stretch" => Some(VideoMode::Stretch),
            "integer" => Some(VideoMode::Integer),
            _ => None,
        }
    }
}

pub fn video_mode_for(root: &Path, stem: &str) -> VideoMode {
    video_mode_for_platform(root, stem, Platform::Gba)
}

pub fn default_for_platform(_platform: Platform) -> VideoMode {
    VideoMode::Actual
}

/// Whether integer scaling is offered for this system: Game Boy and Game Boy Color, on the Brick.
pub fn has_integer(platform: Platform) -> bool {
    cfg!(feature = "brick") && matches!(platform, Platform::Gb | Platform::Gbc)
}

/// The mode a game with no saved choice opens in. `integer` is the Game Boy page's
/// "Integer Scale" setting; it only means something where integer scaling is offered.
pub fn default_scale(platform: Platform, integer: bool) -> VideoMode {
    if integer && has_integer(platform) {
        VideoMode::Integer
    } else {
        default_for_platform(platform)
    }
}

pub fn video_mode_for_platform(root: &Path, stem: &str, platform: Platform) -> VideoMode {
    video_mode_with(root, stem, platform, false)
}

/// As `video_mode_for_platform`, with the Game Boy page's "Integer Scale" setting as the
/// default for a game with no saved choice.
pub fn video_mode_with(root: &Path, stem: &str, platform: Platform, integer: bool) -> VideoMode {
    ini::value(root, VIDEO_MODE_FILE, stem)
        .as_deref()
        .and_then(VideoMode::parse)
        .unwrap_or_else(|| default_scale(platform, integer))
}

pub fn write_video_mode(root: &Path, stem: &str, mode: VideoMode) -> std::io::Result<()> {
    ini::write(root, VIDEO_MODE_FILE, stem, mode.as_str())
}

pub fn source_rect(platform: Platform, mode: VideoMode) -> [f32; 4] {
    source_rect_in(platform, mode, (slot_gfx::OUT_W, slot_gfx::OUT_H))
}

/// The whole multiple of a `w` x `h` picture that fits `panel` (at least 1).
pub fn integer_factor(picture: (u32, u32), panel: (u32, u32)) -> u32 {
    (panel.0 / picture.0.max(1)).min(panel.1 / picture.1.max(1)).max(1)
}

/// As `source_rect`, for a target of `panel` pixels. Integer mode needs the real size: the
/// factor is a whole number of panel pixels per game pixel, not of the 640x480 layout.
pub fn source_rect_in(platform: Platform, mode: VideoMode, panel: (u32, u32)) -> [f32; 4] {
    // Where integer scaling is not offered, a saved "integer" reads as actual size.
    let mode = if mode == VideoMode::Integer && !has_integer(platform) {
        VideoMode::Actual
    } else {
        mode
    };
    let (w, h) = platform.picture();
    if mode == VideoMode::Integer {
        // Show exactly panel / k source pixels, centred on the picture: the picture then
        // covers k panel pixels per game pixel, and the shader paints the rest black.
        let (src_w, src_h) = platform.canvas();
        let k = integer_factor((w, h), panel) as f32;
        let (vw, vh) = (panel.0 as f32 / k, panel.1 as f32 / k);
        return [
            (src_w as f32 - vw) / (2.0 * src_w as f32),
            (src_h as f32 - vh) / (2.0 * src_h as f32),
            vw / src_w as f32,
            vh / src_h as f32,
        ];
    }
    // In canvas units: a NES frame arrives on a 256x240 canvas, the Game Boys' on 240x160.
    let (src_w, src_h) = platform.canvas();
    #[cfg(feature = "brick")]
    if mode == VideoMode::Actual {
        // Expand the source rectangle to 4:3; the shader paints the region
        // outside the active picture black. Stretch mode below has no bars.
        let aspect = slot_gfx::OUT_W as f32 / slot_gfx::OUT_H as f32;
        let (vw, vh) = if w as f32 / h as f32 > aspect {
            (w as f32, w as f32 / aspect)
        } else {
            (h as f32 * aspect, h as f32)
        };
        return [
            (src_w as f32 - vw) / (2.0 * src_w as f32),
            (src_h as f32 - vh) / (2.0 * src_h as f32),
            vw / src_w as f32,
            vh / src_h as f32,
        ];
    }
    if mode == VideoMode::Actual || (w, h) == (src_w, src_h) {
        return WHOLE_TEXTURE;
    }
    let x = src_w.saturating_sub(w) / 2;
    let y = src_h.saturating_sub(h) / 2;
    [
        x as f32 / src_w as f32,
        y as f32 / src_h as f32,
        w as f32 / src_w as f32,
        h as f32 / src_h as f32,
    ]
}

#[cfg(all(test, feature = "brick"))]
mod brick_tests {
    use super::*;
    #[test]
    fn default_fills_screen_without_cropping_game_pixels() {
        assert_eq!(VideoMode::default(), VideoMode::Actual);
        assert_eq!((slot_gfx::OUT_W, slot_gfx::OUT_H), (640, 480));
        assert_eq!((SRC_W, SRC_H), (240, 160));
        for platform in [Platform::Gb, Platform::Gbc, Platform::Gba] {
            let [x, y, w, h] = source_rect(platform, VideoMode::Stretch);
            let (pw, ph) = platform.picture();
            assert!((w * SRC_W as f32 - pw as f32).abs() < 0.001);
            assert!((h * SRC_H as f32 - ph as f32).abs() < 0.001);
            assert!((2.0 * x + w - 1.0).abs() < 0.001);
            assert!((2.0 * y + h - 1.0).abs() < 0.001);
        }
    }
    #[test]
    fn defaults_are_platform_specific_and_saved_choices_work() {
        let root = tempfile::tempdir().unwrap();
        for platform in [Platform::Gb, Platform::Gbc] {
            assert_eq!(
                video_mode_for_platform(root.path(), "unsaved", platform),
                VideoMode::Actual
            );
            let [_, _, w, h] = source_rect(platform, default_for_platform(platform));
            let (pw, ph) = platform.picture();
            let ratio =
                (4.0 / 3.0) * (pw as f32 / (w * SRC_W as f32)) / (ph as f32 / (h * SRC_H as f32));
            assert!((ratio - 10.0 / 9.0).abs() < 0.001);
        }
        assert_eq!(
            video_mode_for_platform(root.path(), "unsaved", Platform::Gba),
            VideoMode::Actual
        );
        assert_eq!(
            source_rect(Platform::Gba, default_for_platform(Platform::Gba)),
            [0.0, -0.0625, 1.0, 1.125]
        );
        write_video_mode(root.path(), "choice", VideoMode::Actual).unwrap();
        assert_eq!(
            video_mode_for_platform(root.path(), "choice", Platform::Gba),
            VideoMode::Actual
        );
    }

    #[test]
    fn a_nes_picture_sits_in_its_own_canvas() {
        let (cw, ch) = Platform::Nes.canvas();
        let [x, y, w, h] = source_rect(Platform::Nes, VideoMode::Stretch);
        assert_eq!((w * cw as f32, h * ch as f32), (256.0, 224.0), "the overscan-cropped picture");
        assert_eq!((x, y * ch as f32), (0.0, 8.0), "centred, 8 lines down");
        let [_, _, w, h] = source_rect(Platform::Nes, VideoMode::Actual);
        assert!((w * cw as f32 / (h * ch as f32) - 4.0 / 3.0).abs() < 0.001, "padded to 4:3");
        assert!((h * ch as f32 - 224.0).abs() < 0.001, "no line cropped");
    }

    #[test]
    fn integer_scale_is_a_whole_number_of_panel_pixels_per_game_pixel() {
        assert_eq!(integer_factor((160, 144), (1024, 768)), 5);
        assert_eq!(integer_factor((160, 144), (640, 480)), 3);
        assert_eq!(integer_factor((160, 144), (100, 100)), 1, "never below 1");
        for platform in [Platform::Gb, Platform::Gbc] {
            for panel in [(1024u32, 768u32), (640, 480)] {
                let [x, y, w, h] = source_rect_in(platform, VideoMode::Integer, panel);
                let (cw, ch) = platform.canvas();
                let (pw, ph) = platform.picture();
                let k = integer_factor((pw, ph), panel) as f32;
                // The window shows panel/k game pixels each way, so a game pixel is k panel pixels.
                assert!((w * cw as f32 * k - panel.0 as f32).abs() < 0.01, "{panel:?} wide");
                assert!((h * ch as f32 * k - panel.1 as f32).abs() < 0.01, "{panel:?} high");
                // Centred on the picture.
                assert!((2.0 * x + w - 1.0).abs() < 0.001);
                assert!((2.0 * y + h - 1.0).abs() < 0.001);
                // The picture stays inside the window, so nothing of it is cropped.
                assert!(w * cw as f32 >= pw as f32 && h * ch as f32 >= ph as f32);
                // And it lands on whole panel pixels (the panel being even here).
                let left = ((cw - pw) as f32 / 2.0 - x * cw as f32) * k;
                assert!((left - left.round()).abs() < 0.01, "{panel:?} picture edge {left}");
            }
        }
    }

    #[test]
    fn integer_scale_is_for_game_boy_only() {
        assert!(has_integer(Platform::Gb) && has_integer(Platform::Gbc));
        for platform in [Platform::Gba, Platform::Nes, Platform::Snes, Platform::Ps1] {
            assert!(!has_integer(platform));
            assert_eq!(
                source_rect(platform, VideoMode::Integer),
                source_rect(platform, VideoMode::Actual),
                "a saved integer reads as actual size where it is not offered"
            );
            assert_eq!(default_scale(platform, true), VideoMode::Actual);
        }
        assert_eq!(default_scale(Platform::Gb, true), VideoMode::Integer);
        assert_eq!(default_scale(Platform::Gbc, false), VideoMode::Actual);
    }

    #[test]
    fn integer_is_saved_and_read_back_and_a_saved_choice_beats_the_setting() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(VideoMode::parse("Integer"), Some(VideoMode::Integer));
        assert_eq!(
            video_mode_with(root.path(), "unsaved", Platform::Gb, true),
            VideoMode::Integer
        );
        write_video_mode(root.path(), "pick", VideoMode::Actual).unwrap();
        assert_eq!(
            video_mode_with(root.path(), "pick", Platform::Gb, true),
            VideoMode::Actual
        );
        write_video_mode(root.path(), "pick", VideoMode::Integer).unwrap();
        assert_eq!(
            video_mode_with(root.path(), "pick", Platform::Gbc, false),
            VideoMode::Integer
        );
    }

    #[test]
    fn actual_mode_preserves_original_aspect() {
        for platform in [Platform::Gb, Platform::Gbc, Platform::Gba] {
            let [_, _, w, h] = source_rect(platform, VideoMode::Actual);
            assert!((w * SRC_W as f32 / (h * SRC_H as f32) - 4.0 / 3.0).abs() < 0.001);
        }
    }
}
