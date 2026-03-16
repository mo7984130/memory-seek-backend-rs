use crate::base::OrtSession;
use crate::types::{BBox, FaceDetection};
use crate::FaceEngineError;

const INPUT_SIZE: u32 = 640;
const SCORE_THRESHOLD: f32 = 0.5;
const NMS_THRESHOLD: f32 = 0.45;
const STRIDES: [u32; 3] = [8, 16, 32];

pub struct Detector {
    session: OrtSession,
}

impl Detector {
    pub fn new(model_path: &str) -> Result<Self, FaceEngineError> {
        let session = OrtSession::new(model_path, INPUT_SIZE, "SCRFD detector")?;
        Ok(Self { session })
    }

    pub fn warmup(&self) -> Result<(), FaceEngineError> {
        self.session.warmup("SCRFD detector")
    }

    pub fn detect(&self, image_bytes: &[u8]) -> Result<Vec<FaceDetection>, FaceEngineError> {
        let img = image::load_from_memory(image_bytes)?;
        let (orig_width, orig_height) = (img.width(), img.height());

        self.session.run_inference(&img, |outputs| {
            Self::postprocess(outputs, orig_width, orig_height)
        })
    }

    fn postprocess(
        outputs: &ort::session::SessionOutputs,
        orig_width: u32,
        orig_height: u32,
    ) -> Result<Vec<FaceDetection>, FaceEngineError> {
        let mut all_detections = Vec::new();

        for (idx, &stride) in STRIDES.iter().enumerate() {
            let feat_size = INPUT_SIZE / stride;

            let (_, scores_data) = outputs[idx]
                .try_extract_tensor::<f32>()
                .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

            let (_, bboxes_data) = outputs[idx + 3]
                .try_extract_tensor::<f32>()
                .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

            let (_, kps_data) = outputs[idx + 6]
                .try_extract_tensor::<f32>()
                .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

            let num_anchors = scores_data.len() / (feat_size as usize * feat_size as usize);

            for (idx_1d, &score) in scores_data.iter().enumerate() {
                if score < SCORE_THRESHOLD {
                    continue;
                }

                let anchor_idx = idx_1d / num_anchors;
                let grid_y = (anchor_idx / feat_size as usize) as f32;
                let grid_x = (anchor_idx % feat_size as usize) as f32;

                let b_idx = idx_1d * 4;
                let x1 = (grid_x - bboxes_data[b_idx]) * stride as f32;
                let y1 = (grid_y - bboxes_data[b_idx + 1]) * stride as f32;
                let x2 = (grid_x + bboxes_data[b_idx + 2]) * stride as f32;
                let y2 = (grid_y + bboxes_data[b_idx + 3]) * stride as f32;

                let bbox_x = (x1 / INPUT_SIZE as f32).clamp(0.0, 1.0);
                let bbox_y = (y1 / INPUT_SIZE as f32).clamp(0.0, 1.0);
                let bbox_w = ((x2 - x1) / INPUT_SIZE as f32).clamp(0.0, 1.0);
                let bbox_h = ((y2 - y1) / INPUT_SIZE as f32).clamp(0.0, 1.0);

                let mut landmarks = [0.0f32; 10];
                let k_idx = idx_1d * 10;
                for k in 0..5 {
                    landmarks[k * 2] = (grid_x + kps_data[k_idx + k * 2]) * stride as f32;
                    landmarks[k * 2 + 1] = (grid_y + kps_data[k_idx + k * 2 + 1]) * stride as f32;
                }

                all_detections.push(FaceDetection::new(
                    BBox { x: bbox_x, y: bbox_y, w: bbox_w, h: bbox_h },
                    landmarks,
                    score,
                ));
            }
        }

        let filtered = Self::apply_nms(all_detections, NMS_THRESHOLD);

        let scale_x = orig_width as f32 / INPUT_SIZE as f32;
        let scale_y = orig_height as f32 / INPUT_SIZE as f32;

        let scaled_detections: Vec<FaceDetection> = filtered
            .into_iter()
            .map(|mut det| {
                for i in 0..5 {
                    det.landmarks[i * 2] *= scale_x;
                    det.landmarks[i * 2 + 1] *= scale_y;
                }
                det
            })
            .collect();

        Ok(scaled_detections)
    }

    fn apply_nms(mut detections: Vec<FaceDetection>, threshold: f32) -> Vec<FaceDetection> {
        if detections.is_empty() {
            return vec![];
        }

        detections.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        let mut selected = Vec::new();

        while !detections.is_empty() {
            let best = detections.remove(0);
            selected.push(best.clone());

            detections.retain(|det| {
                best.bbox.iou(&det.bbox) <= threshold
            });
        }

        selected
    }
}
