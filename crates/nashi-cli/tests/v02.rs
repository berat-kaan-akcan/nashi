//! v0.2 integration tests: EXIF/video orientation, `preview`, `annotateImage`,
//! `organizePDF` rotations, `redactImage`/`redactVideo` styles, and `download`.

mod common;
use common::*;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Create a 100x60 image tagged with EXIF Orientation=6 (RightTop -> visual 60x100).
/// Top-left of the *visual* (oriented) 60x100 image is green; bottom is red.
fn oriented_image(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    // Unoriented 100x60: RightTop (90° CW) maps unoriented (x=0..30, y=0..60)
    // to the top of the oriented 60x100 image.
    tool(
        "magick",
        &[
            "-size",
            "100x60",
            "xc:red",
            "-fill",
            "lime",
            "-draw",
            "rectangle 0,0 29,59",
            "-orient",
            "RightTop",
            s(&p),
        ],
    );
    if p.extension().is_some_and(|e| e == "jpg") && which("exiftool") {
        tool(
            "exiftool",
            &["-q", "-overwrite_original", "-Orientation#=6", s(&p)],
        );
    }
    p
}

fn rotated_mp4(dir: &Path, name: &str) -> PathBuf {
    let raw = mp4(dir, ".raw-unrot.mp4", 0.6, "160x90");
    let out = dir.join(name);
    tool(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-display_rotation",
            "90",
            "-i",
            s(&raw),
            "-c",
            "copy",
            s(&out),
        ],
    );
    out
}

fn pixel_rgb(p: &Path, x: u32, y: u32) -> String {
    let out = Command::new("magick")
        .arg(p)
        .args([
            "-auto-orient",
            "-colorspace",
            "sRGB",
            "-alpha",
            "off",
            "-crop",
            &format!("1x1+{x}+{y}"),
            "-format",
            "%[hex:u.p{0,0}]",
            "INFO:",
        ])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn img_dims(p: &Path) -> (u32, u32) {
    let out = Command::new("magick")
        .arg(p)
        .args(["-auto-orient", "-format", "%w %h", "INFO:"])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    let txt = String::from_utf8_lossy(&out.stdout);
    let mut it = txt.split_whitespace();
    (
        it.next().unwrap().parse().unwrap(),
        it.next().unwrap().parse().unwrap(),
    )
}

#[test]
fn inspect_and_tools_respect_orientation() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let img_name = if which("exiftool") {
        "portrait.jpg"
    } else {
        "portrait.tiff"
    };
    let img = oriented_image(d.path(), img_name);

    let r = run(&["inspect", s(&img), "--json"]);
    assert_eq!(r.code, 0);
    let j: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(j["files"][0]["width"], 60);
    assert_eq!(j["files"][0]["height"], 100);

    // Crop h=80 (> 60 unoriented height): only valid on the auto-oriented 60x100 canvas.
    let c = tool_run(
        "cropImage",
        &[&img],
        Some(r#"{"rect":{"x":0,"y":0,"w":40,"h":80}}"#),
    );
    c.ok();
    assert_eq!(img_dims(&c.one()), (40, 80));

    // Frame adds 10px border around the auto-oriented 60x100 image -> 80x120.
    let f = tool_run(
        "frameImage",
        &[&img],
        Some(r##"{"border":{"px":10,"color":"#0000ff"}}"##),
    );
    f.ok();
    assert_eq!(img_dims(&f.one()), (80, 120));

    if which("ffmpeg") && which("ffprobe") {
        let vid = rotated_mp4(d.path(), "rot90.mp4");
        let rv = run(&["inspect", s(&vid), "--json"]);
        assert_eq!(rv.code, 0);
        let jv: Value = serde_json::from_str(rv.stdout.trim()).unwrap();
        assert_eq!(jv["files"][0]["width"], 90);
        assert_eq!(jv["files"][0]["height"], 160);
    }
    assert_no_temp(d.path());
}

#[test]
fn preview_frame_image_and_video() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let img_name = if which("exiftool") {
        "-oriented.jpg"
    } else {
        "-oriented.tiff"
    };
    let img = oriented_image(d.path(), img_name);
    let out_dir = d.path().join("prev-img");

    // Default width (1600): never upscales 60x100.
    let r = run(&[
        "preview",
        "--kind",
        "frame",
        "--out-dir",
        s(&out_dir),
        "--json",
        "--",
        s(&img),
    ]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(v["kind"], "frame");
    assert_eq!(v["width"], 60);
    assert_eq!(v["height"], 100);
    let f0 = PathBuf::from(v["files"][0].as_str().unwrap());
    assert!(f0.exists());
    assert_eq!(nashi_core::detect::png_dimensions(&f0), Some((60, 100)));

    // Downscale to fit width=30 -> 18x30.
    let out_small = d.path().join("prev-small");
    let r = run(&[
        "preview",
        "--kind",
        "frame",
        "--width",
        "30",
        "--out-dir",
        s(&out_small),
        "--json",
        "--",
        s(&img),
    ]);
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
    assert_eq!(
        (v["width"].as_u64(), v["height"].as_u64()),
        (Some(18), Some(30))
    );

    // Human mode prints summary.
    let h = run(&[
        "preview",
        "--kind",
        "frame",
        "--out-dir",
        s(&out_small),
        "--",
        s(&img),
    ]);
    assert_eq!(h.code, 0);
    assert!(h.stdout.contains("frame (60x100)"));

    if which("ffmpeg") && which("ffprobe") {
        let vid = rotated_mp4(d.path(), "rot90.mp4");
        let out_vid = d.path().join("prev-vid");
        // Test clamping past EOF (--at 999) and negative (--at -2).
        for at in ["-2.0", "0.1", "999.0"] {
            let rv = run(&[
                "preview",
                s(&vid),
                "--kind",
                "frame",
                "--at",
                at,
                "--out-dir",
                s(&out_vid),
                "--json",
            ]);
            assert_eq!(rv.code, 0, "at={at} stderr={}", rv.stderr);
            let vv: Value = serde_json::from_str(rv.stdout.trim()).unwrap();
            assert_eq!(vv["kind"], "frame");
            assert_eq!(vv["width"], 90);
            assert_eq!(vv["height"], 160);
            let pf = PathBuf::from(vv["files"][0].as_str().unwrap());
            assert_eq!(nashi_core::detect::png_dimensions(&pf), Some((90, 160)));
        }
    }
    assert_no_temp(d.path());
}

#[test]
fn preview_waveform_and_pages_and_errors() {
    let d = tempfile::tempdir().unwrap();

    if which("ffmpeg") && which("ffprobe") {
        let w = wav(d.path(), "tone.wav", 1.0);
        let out_w = d.path().join("prev-wave");
        let r = run(&[
            "preview",
            s(&w),
            "--kind",
            "waveform",
            "--out-dir",
            s(&out_w),
            "--json",
        ]);
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["kind"], "waveform");
        assert_eq!(v["width"], 1200);
        assert_eq!(v["height"], 160);
        let wf = PathBuf::from(v["files"][0].as_str().unwrap());
        assert_eq!(nashi_core::detect::png_dimensions(&wf), Some((1200, 160)));

        // Video with no audio stream -> invalid_input (exit 1).
        let vid = mp4(d.path(), "v.mp4", 0.5, "64x64");
        let muted = tool_run("muteVideo", &[&vid], None).one();
        let rm = run(&[
            "preview",
            s(&muted),
            "--kind",
            "waveform",
            "--out-dir",
            s(&out_w),
            "--json",
        ]);
        assert_eq!(rm.code, 1);
        let vm: Value = serde_json::from_str(rm.stdout.trim()).unwrap();
        assert_eq!(vm["error"]["code"], "invalid_input");

        // Bad color -> invalid_options (exit 1).
        let rc = run(&[
            "preview",
            s(&w),
            "--kind",
            "waveform",
            "--color",
            "bad;color",
            "--out-dir",
            s(&out_w),
            "--json",
        ]);
        assert_eq!(rc.code, 1);
        let vc: Value = serde_json::from_str(rc.stdout.trim()).unwrap();
        assert_eq!(vc["error"]["code"], "invalid_options");

        // Missing ffmpeg -> exit 3.
        let rmiss = run_env(
            &[
                "preview",
                s(&w),
                "--kind",
                "waveform",
                "--out-dir",
                s(&out_w),
                "--json",
            ],
            &[("NASHI_DISABLE_TOOLS", "ffmpeg")],
        );
        assert_eq!(rmiss.code, 3);
        let vmiss: Value = serde_json::from_str(rmiss.stdout.trim()).unwrap();
        assert_eq!(vmiss["error"]["code"], "missing_dependency");
    }

    if which("pdftoppm") {
        let p = pdf(d.path(), "doc.pdf", 3);
        let out_p = d.path().join("prev-pages");
        let r = run(&[
            "preview",
            s(&p),
            "--kind",
            "pages",
            "--out-dir",
            s(&out_p),
            "--json",
        ]);
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        let v: Value = serde_json::from_str(r.stdout.trim()).unwrap();
        assert_eq!(v["kind"], "pages");
        assert_eq!(v["width"], 240);
        let files = v["files"].as_array().unwrap();
        assert_eq!(files.len(), 3);
        assert!(files[0].as_str().unwrap().ends_with("/page-001.png"));
        assert!(files[2].as_str().unwrap().ends_with("/page-003.png"));
        for f in files {
            assert!(Path::new(f.as_str().unwrap()).exists());
        }

        // Subset and custom order with --pages "3,1" and --width 120.
        let out_sub = d.path().join("prev-sub");
        let rs = run(&[
            "preview",
            s(&p),
            "--kind",
            "pages",
            "--pages",
            "3,1",
            "--width",
            "120",
            "--out-dir",
            s(&out_sub),
            "--json",
        ]);
        assert_eq!(rs.code, 0);
        let vs: Value = serde_json::from_str(rs.stdout.trim()).unwrap();
        assert_eq!(vs["width"], 120);
        assert_eq!(vs["files"].as_array().unwrap().len(), 2);

        // Missing pdftoppm -> exit 3.
        let rmiss = run_env(
            &[
                "preview",
                s(&p),
                "--kind",
                "pages",
                "--out-dir",
                s(&out_sub),
                "--json",
            ],
            &[("NASHI_DISABLE_TOOLS", "pdftoppm")],
        );
        assert_eq!(rmiss.code, 3);
        let vmiss: Value = serde_json::from_str(rmiss.stdout.trim()).unwrap();
        assert_eq!(vmiss["error"]["code"], "missing_dependency");
    }

    // Usage error -> exit 2, empty stdout.
    let ru = run(&["preview", "--json"]);
    assert_eq!(ru.code, 2);
    assert!(ru.stdout.is_empty());
    assert_no_temp(d.path());
}

#[test]
fn annotate_image_and_security_rejections() {
    need!("magick");
    let d = tempfile::tempdir().unwrap();
    let base = png(d.path(), "photo.png"); // 64x48: left half blue, right half red
    let before = hash(&base);

    // 32x24 overlay (half resolution -> scaled up to 64x48): top-right painted yellow.
    let ov = d.path().join("overlay.png");
    tool(
        "magick",
        &[
            "-size",
            "32x24",
            "xc:none",
            "-fill",
            "#00ff00",
            "-draw",
            "rectangle 20,0 31,10",
            s(&ov),
        ],
    );
    let ov_before = hash(&ov);

    let r = tool_run(
        "annotateImage",
        &[&base],
        Some(&format!(r#"{{"overlay":"{}"}}"#, s(&ov))),
    );
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("photo-annotated.png"));
    assert_eq!(img_dims(&out), (64, 48));
    // Scaled overlay painted top-right green; bottom-right stayed red.
    let top_right = pixel_rgb(&out, 50, 5);
    let bot_right = pixel_rgb(&out, 50, 40);
    assert_ne!(top_right, bot_right);
    assert_eq!(hash(&base), before);
    assert_eq!(hash(&ov), ov_before);

    // Rejections: same as input, hardlink to input, symlink, non-png, fake png, directory, missing.
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&base)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    let hard = d.path().join("hardlink.png");
    std::fs::hard_link(&base, &hard).unwrap();
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&hard)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    let sym = d.path().join("sym.png");
    std::os::unix::fs::symlink(&ov, &sym).unwrap();
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&sym)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    let j = jpg(d.path(), "not-png.jpg");
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&j)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    let fake = text(d.path(), "fake.png", "not a png header");
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&fake)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    let dir_png = d.path().join("dir.png");
    std::fs::create_dir(&dir_png).unwrap();
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(&format!(r#"{{"overlay":"{}"}}"#, s(&dir_png)))
        )
        .error_codes(),
        ["invalid_options"]
    );
    assert_eq!(
        tool_run(
            "annotateImage",
            &[&base],
            Some(r#"{"overlay":"/nonexistent/overlay.png"}"#)
        )
        .error_codes(),
        ["invalid_options"]
    );
    assert_no_temp(d.path());
}

#[test]
fn organize_pdf_with_rotations() {
    need!("qpdf", "pdfinfo");
    let d = tempfile::tempdir().unwrap();
    let p = pdf(d.path(), "doc.pdf", 3);

    let r = tool_run(
        "organizePDF",
        &[&p],
        Some(r#"{"pages":"3,1,2","rotations":[0,90,-90]}"#),
    );
    r.ok();
    let out = r.one();
    assert_eq!(out, d.path().join("doc-organized.pdf"));

    let info = Command::new("pdfinfo")
        .args(["-f", "1", "-l", "3"])
        .arg(&out)
        .output()
        .unwrap();
    let txt = String::from_utf8_lossy(&info.stdout);
    let rots: Vec<u32> = txt
        .lines()
        .filter_map(|l| {
            let (_, rhs) = l.split_once("rot:")?;
            rhs.trim().parse().ok()
        })
        .collect();
    assert_eq!(rots, vec![0, 90, 270], "pdfinfo:\n{txt}");

    if which("pdftotext") {
        let pt = Command::new("pdftotext")
            .arg(&out)
            .arg("-")
            .output()
            .unwrap();
        let body = String::from_utf8_lossy(&pt.stdout);
        assert!(body.find("Page 3").unwrap() < body.find("Page 1").unwrap());
        assert!(body.find("Page 1").unwrap() < body.find("Page 2").unwrap());
    }

    // Validation failures: mismatched length & non-90 degree multiple.
    assert_eq!(
        tool_run(
            "organizePDF",
            &[&p],
            Some(r#"{"pages":"1,2,3","rotations":[0,90]}"#)
        )
        .error_codes(),
        ["invalid_options"]
    );
    assert_eq!(
        tool_run(
            "organizePDF",
            &[&p],
            Some(r#"{"pages":"1,2","rotations":[0,45]}"#)
        )
        .error_codes(),
        ["invalid_options"]
    );

    // Without qpdf, non-zero rotation returns missing_dependency (exit 3).
    let rmiss = tool_run_env(
        "organizePDF",
        &[&p],
        Some(r#"{"pages":"1,2","rotations":[0,90]}"#),
        &[("NASHI_DISABLE_TOOLS", "qpdf")],
    );
    assert_eq!(rmiss.code, 3);
    assert_eq!(rmiss.error_codes(), ["missing_dependency"]);
    assert_no_temp(d.path());
}

#[test]
fn redact_image_and_video_styles() {
    let d = tempfile::tempdir().unwrap();

    if which("magick") {
        let chk = d.path().join("check.png");
        tool(
            "magick",
            &["-size", "96x72", "pattern:checkerboard", s(&chk)],
        );
        let orig_px = pixel_rgb(&chk, 45, 25);

        let r = tool_run(
            "redactImage",
            &[&chk],
            Some(
                r#"{"rects":[
                    {"x":0,"y":0,"w":20,"h":20,"style":"black"},
                    {"x":30,"y":10,"w":30,"h":30,"style":"blur"},
                    {"x":65,"y":20,"w":40,"h":60,"style":"pixelate"}
                ]}"#,
            ),
        );
        r.ok();
        let out = r.one();
        assert_eq!(img_dims(&out), (96, 72));
        assert_eq!(pixel_rgb(&out, 5, 5), "000000");
        let blurred_px = pixel_rgb(&out, 45, 25);
        assert_ne!(blurred_px, orig_px);
        assert_ne!(blurred_px, "000000");
        // The pixelate rect runs past the image edge and must still be accepted;
        // tests/behavior.rs checks that pixelation really removes detail.
        assert_ne!(pixel_rgb(&out, 66, 21), "000000");

        assert_eq!(
            tool_run(
                "redactImage",
                &[&chk],
                Some(r#"{"rects":[{"x":0,"y":0,"w":10,"h":10,"style":"bogus"}]}"#)
            )
            .error_codes(),
            ["invalid_options"]
        );
    }

    if which("ffmpeg") && which("ffprobe") {
        let vid = mp4(d.path(), "clip.mp4", 1.0, "160x120");
        let r = tool_run(
            "redactVideo",
            &[&vid],
            Some(
                r#"{"rects":[
                    {"x":0,"y":0,"w":20,"h":20,"t0":0.0,"t1":0.5,"style":"black"},
                    {"x":25,"y":15,"w":35,"h":35,"t0":0.2,"t1":0.8,"style":"blur"},
                    {"x":100,"y":80,"w":90,"h":80,"style":"pixelate"},
                    {"x":5,"y":5,"w":1,"h":1,"style":"blur"}
                ]}"#,
            ),
        );
        r.ok();
        let out = r.one();
        assert_eq!(out, d.path().join("clip-redacted.mp4"));
        let j = ffprobe_json(&out);
        assert_eq!(j["streams"][0]["width"], 160);
        assert_eq!(j["streams"][0]["height"], 120);

        assert_eq!(
            tool_run(
                "redactVideo",
                &[&vid],
                Some(r#"{"rects":[{"x":0,"y":0,"w":10,"h":10,"style":"bogus"}]}"#)
            )
            .error_codes(),
            ["invalid_options"]
        );
    }
    assert_no_temp(d.path());
}

#[test]
fn download_validation_missing_deps_and_local_http() {
    let d = tempfile::tempdir().unwrap();
    let out_dir = d.path().join("dl");

    // Non-http(s) schemes and malformed hosts -> invalid_input event (exit 1).
    for bad in [
        "file:///etc/passwd",
        "ftp://127.0.0.1/a.png",
        "javascript:alert(1)",
        "http://",
        "https:///etc/passwd",
        "-o/tmp/evil",
    ] {
        let r = run(&["download", "--out-dir", s(&out_dir), "--json", "--", bad]);
        assert_eq!(r.code, 1, "url={bad}");
        assert_eq!(r.error_codes(), ["invalid_input"], "url={bad}");
        assert_eq!(r.errors()[0]["input"], bad);
        assert_eq!(r.errors()[0]["index"], 0);
        assert_eq!(r.done()["failed"], 1);
    }

    // Missing curl / yt-dlp -> exit 3 with missing_dependency.
    let rc = run_env(
        &[
            "download",
            "https://example.com/pic.png",
            "--out-dir",
            s(&out_dir),
            "--json",
        ],
        &[("NASHI_DISABLE_TOOLS", "curl")],
    );
    assert_eq!(rc.code, 3);
    assert_eq!(rc.error_codes(), ["missing_dependency"]);

    let ry = run_env(
        &[
            "download",
            "https://example.com/watch?v=123",
            "--out-dir",
            s(&out_dir),
            "--json",
        ],
        &[("NASHI_DISABLE_TOOLS", "yt-dlp")],
    );
    assert_eq!(ry.code, 3);
    assert_eq!(ry.error_codes(), ["missing_dependency"]);

    if !which("curl") || !which("magick") {
        eprintln!("skipped: curl/magick missing for local HTTP download test");
        return;
    }

    // Local 127.0.0.1 HTTP server serving PNG bytes and a hostile file:// redirect.
    let sample_png = png(d.path(), "src.png");
    let png_bytes = std::fs::read(&sample_png).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_bytes = png_bytes.clone();
    // Nothing may ever connect here: curl follows FTP redirects by default, and
    // `--proto` / `--proto-redir` each stop that (removing both fails this test).
    let ftp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    ftp_listener.set_nonblocking(true).unwrap();
    let ftp_port = ftp_listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        // Serve 6 sequential requests.
        for _ in 0..6 {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut req_buf = [0u8; 2048];
            let n = stream.read(&mut req_buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&req_buf[..n]);
            let first_line = req.lines().next().unwrap_or("");
            if first_line.contains("/redirect-file") {
                let resp = "HTTP/1.1 302 Found\r\nLocation: file:///etc/hostname\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
            } else if first_line.contains("/redirect-ftp") {
                let resp = format!("HTTP/1.1 302 Found\r\nLocation: ftp://127.0.0.1:{ftp_port}/x.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(resp.as_bytes());
            } else {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    server_bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&server_bytes);
            }
            let _ = stream.flush();
        }
    });

    // 1. `--kind auto` on `.png` URL with query string + percent-encoding.
    let url1 = format!("http://127.0.0.1:{port}/files/My%20Cat.png?token=xyz");
    let r1 = run(&["download", &url1, "--out-dir", s(&out_dir), "--json"]);
    r1.ok();
    assert_eq!(r1.one(), out_dir.join("My Cat.png"));
    assert_eq!(std::fs::read(r1.one()).unwrap(), png_bytes);

    // 2. Duplicate download never overwrites -> `My Cat (1).png`.
    let r2 = run(&["download", &url1, "--out-dir", s(&out_dir), "--json"]);
    r2.ok();
    assert_eq!(r2.one(), out_dir.join("My Cat (1).png"));

    // 3. Hostile leading `-` in URL filename is stripped (`-evil.png` -> `evil.png`).
    let url3 = format!("http://127.0.0.1:{port}/-evil.png");
    let r3 = run(&[
        "download",
        &url3,
        "--kind",
        "image",
        "--out-dir",
        s(&out_dir),
        "--json",
    ]);
    r3.ok();
    assert_eq!(r3.one(), out_dir.join("evil.png"));

    // 4. Extensionless URL with `--kind image` uses Content-Type (`image/png`) + XDG_DOWNLOAD_DIR default.
    let xdg_dir = d.path().join("xdg-downloads");
    let url4 = format!("http://127.0.0.1:{port}/avatar?id=7");
    let r4 = run_env(
        &["download", &url4, "--kind", "image", "--json"],
        &[("XDG_DOWNLOAD_DIR", s(&xdg_dir))],
    );
    r4.ok();
    assert_eq!(r4.one(), xdg_dir.join("avatar.png"));

    // 5. Redirect to `file://` never yields a file (curl refuses this by default too).
    let url5 = format!("http://127.0.0.1:{port}/redirect-file.png");
    let r5 = run(&["download", &url5, "--out-dir", s(&out_dir), "--json"]);
    assert_eq!(r5.code, 1);
    assert_eq!(r5.error_codes(), ["tool_failed"]);

    // 6. Redirect to `ftp://` is refused before connecting.
    let url6 = format!("http://127.0.0.1:{port}/redirect-ftp.png");
    let r6 = run(&["download", &url6, "--out-dir", s(&out_dir), "--json"]);
    assert_eq!(r6.code, 1);
    assert_eq!(r6.error_codes(), ["tool_failed"]);
    assert!(
        ftp_listener.accept().is_err(),
        "curl followed a redirect to ftp://"
    );

    let _ = handle.join();
    assert_no_temp(d.path());
}

extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[test]
fn download_ytdlp_local_and_cancellation() {
    let d = tempfile::tempdir().unwrap();
    let out_dir = d.path().join("ytdl");

    if which("yt-dlp") && which("ffmpeg") {
        let clip = mp4(d.path(), "src.mp4", 0.6, "120x90");
        let clip_bytes = std::fs::read(&clip).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(false).unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop2 = std::sync::Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut req_buf = [0u8; 2048];
                let n = stream.read(&mut req_buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&req_buf[..n]);
                let is_head = req.starts_with("HEAD ");
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    clip_bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                if !is_head {
                    let _ = stream.write_all(&clip_bytes);
                }
                let _ = stream.flush();
            }
        });

        let url = format!("http://127.0.0.1:{port}/Demo%20Clip.mp4");
        let rv = run(&[
            "download",
            &url,
            "--kind",
            "video",
            "--out-dir",
            s(&out_dir),
            "--json",
        ]);
        rv.ok();
        assert_eq!(rv.one(), out_dir.join("Demo Clip.mp4"));
        assert!(rv
            .events
            .iter()
            .any(|e| e["event"] == "progress" && e["fraction"].is_f64()));

        let ra = run(&[
            "download",
            &url,
            "--kind",
            "audio",
            "--out-dir",
            s(&out_dir),
            "--json",
        ]);
        ra.ok();
        assert_eq!(ra.one(), out_dir.join("Demo Clip.mp3"));

        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = std::net::TcpStream::connect(("127.0.0.1", port));
        let _ = handle.join();
    }

    if which("curl") {
        let cancel_dir = d.path().join("cancel-dl");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut req_buf = [0u8; 1024];
                let _ = stream.read(&mut req_buf);
                let header =
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 1000000\r\n\r\n";
                let _ = stream.write_all(header.as_bytes());
                let chunk = vec![b'x'; 50_000];
                for _ in 0..20 {
                    if stream.write_all(&chunk).is_err() || stream.flush().is_err() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            }
        });

        let url = format!("http://127.0.0.1:{port}/slow.png");
        let mut child = Command::new(BIN)
            .args([
                "download",
                &url,
                "--kind",
                "image",
                "--out-dir",
                s(&cancel_dir),
                "--json",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();

        use std::io::{BufRead, BufReader};
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            let v: Value = serde_json::from_str(line.trim()).unwrap();
            check_event(&v);
            if v["event"] == "progress" && v["fraction"].as_f64().is_some_and(|f| f > 0.0) {
                break;
            }
        }
        unsafe {
            libc_kill(child.id() as i32, 15);
        }
        let mut rest = String::new();
        reader.read_to_string(&mut rest).unwrap();
        let status = child.wait().unwrap();
        assert_eq!(status.code(), Some(130));
        let last: Value = serde_json::from_str(rest.lines().last().unwrap()).unwrap();
        check_event(&last);
        assert_eq!(last["event"], "done");
        assert_eq!(last["cancelled"], true);
        let _ = handle.join();
        assert_no_temp(&cancel_dir);
        assert!(!cancel_dir.join("slow.png").exists());
    }
    assert_no_temp(d.path());
}
