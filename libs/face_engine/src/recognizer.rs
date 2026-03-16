use crate::base::OrtSession;
use crate::FaceEngineError;
use image::DynamicImage;

const INPUT_SIZE: u32 = 112;
const EMBEDDING_DIM: usize = 512;

pub struct Recognizer {
    session: OrtSession,
}

impl Recognizer {
    pub fn new(model_path: &str) -> Result<Self, FaceEngineError> {
        let session = OrtSession::new(model_path, INPUT_SIZE, "ArcFace recognizer")?;
        Ok(Self { session })
    }

    pub fn warmup(&self) -> Result<(), FaceEngineError> {
        self.session.warmup("ArcFace recognizer")
    }

    pub fn extract(&self, aligned_face: &DynamicImage) -> Result<[f32; 512], FaceEngineError> {
        self.session.run_inference(aligned_face, |outputs| {
            Self::postprocess(outputs)
        })
    }

    fn postprocess(outputs: &ort::session::SessionOutputs) -> Result<[f32; 512], FaceEngineError> {
        let (_, output_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| FaceEngineError::OrtError(e.to_string()))?;

        if output_data.len() != EMBEDDING_DIM {
            return Err(FaceEngineError::InvalidOutput(format!(
                "Expected embedding dimension {}, got {}",
                EMBEDDING_DIM,
                output_data.len()
            )));
        }

        let mut embedding = [0.0f32; 512];
        embedding.copy_from_slice(output_data);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for e in embedding.iter_mut() {
                *e /= norm;
            }
        }

        Ok(embedding)
    }
}
