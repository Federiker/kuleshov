#![expect(
    clippy::cast_precision_loss,
    clippy::items_after_statements,
    reason = "pixel positions are small; the clean-up guard sits where it is used"
)]

use super::*;
use crate::support::tools;
use std::fs;

#[test]
fn motions_use_output_frames_and_reach_the_final_position() {
    for direction in [
        Direction::ZoomIn,
        Direction::ZoomOut,
        Direction::LeftToRight,
        Direction::RightToLeft,
    ] {
        let (filter, frames) = motion_filter(1080, 1920, 6.0, direction, None, 30).unwrap();
        assert_eq!(frames, 180);
        assert!(filter.contains("scale=4320:7680"));
        assert!(filter.contains("on/179"));
        assert!(filter.contains("d=180:s=1080x1920:fps=30"));
    }
}

#[test]
fn tiny_clips_have_at_least_one_frame_without_dividing_by_zero() {
    let (filter, frames) = motion_filter(90, 160, 0.0, Direction::ZoomIn, None, 1).unwrap();
    assert_eq!(frames, 1);
    assert!(filter.contains("on/1"));
}

#[test]
fn invalid_motion_parameters_are_errors() {
    for (w, h, duration, zoom, fps) in [
        (0, 160, 1.0, None, 30),
        (91, 160, 1.0, None, 30),
        (90, 160, f64::NAN, None, 30),
        (90, 160, f64::INFINITY, None, 30),
        (90, 160, 1.0, Some(f64::NAN), 30),
        (90, 160, 1.0, None, 0),
        (10000, 10000, 1.0, None, 30),
    ] {
        assert!(motion_filter(w, h, duration, Direction::ZoomIn, zoom, fps).is_err());
    }
}

#[test]

fn supersampled_motion_reduces_zoom_jitter_and_pans_matching_aspect_images() {
    let dir = std::env::temp_dir().join(format!("kuleshov-motion-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let image = dir.join("marker.ppm");
    let mut pixels = vec![30_u8; 1080 * 1920 * 3];
    for y in 400..500 {
        for x in 200..300 {
            pixels[(y * 1080 + x) * 3..(y * 1080 + x + 1) * 3].fill(255);
        }
    }
    let mut data = b"P6\n1080 1920\n255\n".to_vec();
    data.extend(pixels);
    fs::write(&image, data).unwrap();

    let positions = |filter: &str| {
        let output = tools()
            .command(crate::Tool::Ffmpeg)
            .args(["-v", "error", "-loop", "1", "-framerate", "30", "-i"])
            .arg(&image)
            .arg("-vf")
            .arg(format!("{filter},crop=400:400:20:150,format=gray"))
            .args([
                "-frames:v",
                "180",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "gray",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 180 * 400 * 400);
        output
            .stdout
            .chunks_exact(400 * 400)
            .map(|frame| {
                let (mut weight, mut sum_x) = (0.0, 0.0);
                for (i, pixel) in frame.iter().enumerate() {
                    let value = f64::from(pixel.saturating_sub(35));
                    weight += value;
                    sum_x += (i % 400) as f64 * value;
                }
                assert!(weight > 0.0);
                sum_x / weight
            })
            .collect::<Vec<_>>()
    };
    let variation = |points: &[f64]| {
        let steps: Vec<_> = points.windows(2).map(|w| w[1] - w[0]).collect();
        let mean = steps.iter().sum::<f64>() / steps.len() as f64;
        (steps.iter().map(|step| (step - mean).powi(2)).sum::<f64>() / steps.len() as f64).sqrt()
    };
    for direction in [
        Direction::ZoomIn,
        Direction::ZoomOut,
        Direction::LeftToRight,
        Direction::RightToLeft,
    ] {
        let (filter, _) = motion_filter(1080, 1920, 6.0, direction, None, 30).unwrap();
        let points = positions(&filter);
        let sign = match direction {
            Direction::ZoomIn | Direction::LeftToRight => -1.0,
            _ => 1.0,
        };
        assert!(
            points.windows(2).all(|w| (w[1] - w[0]) * sign > 0.0),
            "{direction:?} must move on every frame without reversals"
        );
        if matches!(direction, Direction::ZoomIn | Direction::ZoomOut) {
            let z = if direction == Direction::ZoomIn {
                "1+0.25*on/179"
            } else {
                "1.25-0.25*on/179"
            };
            let old = positions(&format!(
                "scale=1080:1920,setsar=1,zoompan=z='{z}':x='(iw-iw/zoom)/2':y='(ih-ih/zoom)/2':d=180:s=1080x1920:fps=30"
            ));
            assert!(
                variation(&points) < variation(&old) * 0.4,
                "{direction:?}: {} vs {}",
                variation(&points),
                variation(&old)
            );
        }
    }
}

#[test]
fn a_clip_fades_in_at_its_start_and_out_before_its_last_frame() {
    let both = ClipFades {
        fade_in: 0.5,
        fade_out: 0.5,
    };
    // 90 frames at 30 FPS: three seconds, so the fade out starts at 2.5 s.
    assert_eq!(
        fade_filter(both, 90, 30),
        ",fade=t=in:st=0:d=0.5,fade=t=out:st=2.5:d=0.5"
    );
    let first = ClipFades {
        fade_in: 0.0,
        fade_out: 0.4,
    };
    assert_eq!(fade_filter(first, 45, 30), ",fade=t=out:st=1.1:d=0.4");
    let none = ClipFades {
        fade_in: 0.0,
        fade_out: 0.0,
    };
    assert_eq!(fade_filter(none, 45, 30), "");
    // A fade longer than the clip starts at its beginning.
    let long = ClipFades {
        fade_in: 0.0,
        fade_out: 5.0,
    };
    assert_eq!(fade_filter(long, 30, 30), ",fade=t=out:st=0:d=5");
}
