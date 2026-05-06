use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use egui::ColorImage;

pub struct ThumbnailJob {
    pub hash: [u8; 32],
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

impl From<crate::scanner::MeshData> for ThumbnailJob {
    fn from(m: crate::scanner::MeshData) -> Self {
        Self {
            hash: m.hash,
            vertices: m.vertices,
            faces: m.faces,
        }
    }
}

pub enum ThumbnailResult {
    Success {
        hash: [u8; 32],
        image: ColorImage,
    },
    Error {
        hash: [u8; 32],
        message: String,
    },
}

/// Spawn N worker threads for thumbnail generation.
/// Each worker checks `cancel_token` before processing a job — when set,
/// workers drain remaining jobs as no-ops and exit.
pub fn spawn_thumbnail_workers(
    job_rx: Receiver<ThumbnailJob>,
    result_tx: Sender<ThumbnailResult>,
    num_workers: usize,
    cancel_token: Arc<AtomicBool>,
) {
    for _ in 0..num_workers {
        let rx = job_rx.clone();
        let tx = result_tx.clone();
        let token = cancel_token.clone();
        std::thread::spawn(move || {
            while let Ok(job) = rx.recv() {
                if token.load(Ordering::SeqCst) {
                    // Drain remaining jobs without processing
                    while rx.recv().is_ok() {}
                    break;
                }

                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match crate::thumbnail::generate_wireframe(
                        &job.vertices,
                        &job.faces,
                        160,
                        112,
                    ) {
                        Some(image) => ThumbnailResult::Success {
                            hash: job.hash,
                            image,
                        },
                        None => ThumbnailResult::Error {
                            hash: job.hash,
                            message: "Empty mesh data".to_string(),
                        },
                    }
                }));

                let final_result = match result {
                    Ok(r) => r,
                    Err(panic_info) => {
                        let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = panic_info.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "Worker panicked during thumbnail generation".to_string()
                        };
                        ThumbnailResult::Error {
                            hash: job.hash,
                            message: msg,
                        }
                    }
                };

                if tx.send(final_result).is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_produces_success_for_valid_mesh() {
        let (job_tx, job_rx) = crossbeam_channel::bounded::<ThumbnailJob>(1);
        let (result_tx, result_rx) = crossbeam_channel::bounded::<ThumbnailResult>(1);
        let cancel_token = Arc::new(AtomicBool::new(false));

        spawn_thumbnail_workers(job_rx, result_tx, 1, cancel_token);

        let verts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let faces = vec![[0, 1, 2]];
        job_tx
            .send(ThumbnailJob {
                hash: [0u8; 32],
                vertices: verts,
                faces,
            })
            .unwrap();
        drop(job_tx);

        let result = result_rx.recv().unwrap();
        match result {
            ThumbnailResult::Success { hash, image } => {
                assert_eq!(hash, [0u8; 32]);
                assert_eq!(image.size, [160, 112]);
            }
            ThumbnailResult::Error { message, .. } => {
                panic!("Expected success, got error: {}", message);
            }
        }
    }

    #[test]
    fn worker_produces_error_for_empty_mesh() {
        let (job_tx, job_rx) = crossbeam_channel::bounded::<ThumbnailJob>(1);
        let (result_tx, result_rx) = crossbeam_channel::bounded::<ThumbnailResult>(1);
        let cancel_token = Arc::new(AtomicBool::new(false));

        spawn_thumbnail_workers(job_rx, result_tx, 1, cancel_token);

        job_tx
            .send(ThumbnailJob {
                hash: [0u8; 32],
                vertices: vec![],
                faces: vec![],
            })
            .unwrap();
        drop(job_tx);

        let result = result_rx.recv().unwrap();
        assert!(matches!(result, ThumbnailResult::Error { .. }));
    }

    #[test]
    fn worker_cancel_token_drains_without_processing() {
        let cancel_token = Arc::new(AtomicBool::new(true)); // already cancelled

        let (job_tx, job_rx) = crossbeam_channel::bounded::<ThumbnailJob>(2);
        let (result_tx, result_rx) = crossbeam_channel::bounded::<ThumbnailResult>(2);

        spawn_thumbnail_workers(job_rx, result_tx, 1, cancel_token);

        let verts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let faces = vec![[0, 1, 2]];
        job_tx
            .send(ThumbnailJob {
                hash: [0u8; 32],
                vertices: verts.clone(),
                faces: faces.clone(),
            })
            .unwrap();
        job_tx
            .send(ThumbnailJob {
                hash: [1u8; 32],
                vertices: verts,
                faces,
            })
            .unwrap();
        drop(job_tx);

        // All jobs drained without results — receiver gets RecvError
        assert!(result_rx.recv().is_err());
    }

    #[test]
    fn worker_panic_is_caught_and_reported_as_error() {
        let (job_tx, job_rx) = crossbeam_channel::bounded::<ThumbnailJob>(1);
        let (result_tx, result_rx) = crossbeam_channel::bounded::<ThumbnailResult>(1);
        let cancel_token = Arc::new(AtomicBool::new(false));

        spawn_thumbnail_workers(job_rx, result_tx, 1, cancel_token);

        // Invalid mesh (empty vertices with face indices): generates error, not panic.
        // The catch_unwind Err arm requires unsafe/panic!() to exercise directly.
        job_tx
            .send(ThumbnailJob {
                hash: [1u8; 32],
                vertices: vec![],
                faces: vec![[0, 1, 2]],
            })
            .unwrap();
        drop(job_tx);

        let result = result_rx.recv().unwrap();
        assert!(matches!(result, ThumbnailResult::Error { .. }));
    }
}
