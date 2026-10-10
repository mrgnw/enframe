#![forbid(unsafe_code)]
use std::sync::OnceLock;

pub mod graph;
pub mod net;
pub mod onnx;
pub mod yunet;
pub mod manifest {
    pub const MAX_MODEL_BYTES: u64 = 64 << 20;
}

pub const YUNET_MODEL_SHA256: &str =
    "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";
pub static YUNET_MODEL: &[u8] = include_bytes!("../model/face_detection_yunet_2023mar.onnx");

pub fn detector() -> Result<&'static yunet::Detector, yunet::DetectError> {
    static DETECTOR: OnceLock<Result<yunet::Detector, yunet::DetectError>> = OnceLock::new();
    match DETECTOR.get_or_init(|| yunet::Detector::new(YUNET_MODEL)) {
        Ok(detector) => Ok(detector),
        Err(error) => Err(yunet::DetectError::Model(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::{YUNET_MODEL, YUNET_MODEL_SHA256};
    use sha2::{Digest, Sha256};

    #[test]
    fn bundled_model_has_the_pinned_sha256() {
        assert_eq!(
            format!("{:x}", Sha256::digest(YUNET_MODEL)),
            YUNET_MODEL_SHA256
        );
    }
}
