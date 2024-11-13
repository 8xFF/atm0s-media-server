mod codec;
mod composer;
mod transmuxer;

pub use codec::*;
pub use composer::*;
pub use transmuxer::*;

#[derive(Debug, Clone)]
pub enum RecordConvertOutputLocation {
    S3(String),
    Local(String),
}

pub struct RecordConvertConfig {
    pub in_s3: String,
    pub transmux: Option<RecordConvertOutputLocation>,
    pub compose: Option<RecordComposerConfig>,
}

#[derive(Debug, Clone)]
pub struct RecordConvertOutput {
    pub transmux: Option<TransmuxSummary>,
    pub compose: Option<RecordComposerResult>,
}

pub struct RecordConvert {
    cfg: RecordConvertConfig,
}

impl RecordConvert {
    pub fn new(cfg: RecordConvertConfig) -> Self {
        Self { cfg }
    }

    pub async fn convert(self) -> Result<RecordConvertOutput, String> {
        let mut transmux = None;
        if let Some(out) = self.cfg.transmux {
            let transmuxer = RecordTransmuxer::new(self.cfg.in_s3.clone(), out);
            transmux = Some(transmuxer.convert().await?);
        }
        let mut compose = None;
        if let Some(cfg) = self.cfg.compose.as_ref() {
            if cfg.audio || cfg.video {
                let composer = RecordComposer::new(self.cfg.in_s3.clone(), cfg.clone());
                compose = Some(composer.compose().await?);
            }
        }
        Ok(RecordConvertOutput { transmux, compose })
    }
}
