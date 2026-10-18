//! Catalog of downloadable on-device models and a verified downloader.
//!
//! Every file is pinned by byte size and SHA-256. Downloads stream into a
//! `.part` file and are only renamed into place after verification.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    /// Speech-to-text (whisper.cpp GGML)
    Speech,
    /// Small language model for understanding tasks (GGUF)
    Language,
}

pub struct ModelFile {
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub struct ModelSpec {
    pub id: &'static str,
    pub kind: ModelKind,
    pub name: &'static str,
    pub blurb: &'static str,
    /// All files needed for this model; the first one is the weights.
    pub files: &'static [ModelFile],
}

impl ModelSpec {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    pub fn path(&self, index: usize) -> PathBuf {
        models_dir().join(self.files[index].file)
    }

    pub fn is_installed(&self) -> bool {
        self.files.iter().all(|f| {
            std::fs::metadata(models_dir().join(f.file))
                .map(|m| m.len() == f.size)
                .unwrap_or(false)
        })
    }

    pub fn remove(&self) -> std::io::Result<()> {
        for (i, f) in self.files.iter().enumerate() {
            // The Qwen tokenizer is shared between language models; keep it if still needed.
            if self.kind == ModelKind::Language && i > 0 {
                let shared = CATALOG.iter().any(|m| {
                    m.id != self.id && m.is_installed() && m.files.iter().any(|o| o.file == f.file)
                });
                if shared {
                    continue;
                }
            }
            let path = models_dir().join(f.file);
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

const QWEN_TOKENIZER: ModelFile = ModelFile {
    file: "qwen2.5-tokenizer.json",
    url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/7ae557604adf67be50417f59c2c2f167def9a775/tokenizer.json",
    size: 7_031_645,
    sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
};

pub static CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "whisper-tiny.en",
        kind: ModelKind::Speech,
        name: "Whisper Tiny",
        blurb: "Fastest, fine for short notes",
        files: &[ModelFile {
            file: "ggml-tiny.en.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
            size: 77_704_715,
            sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        }],
    },
    ModelSpec {
        id: "whisper-base.en",
        kind: ModelKind::Speech,
        name: "Whisper Base",
        blurb: "Recommended balance of speed and accuracy",
        files: &[ModelFile {
            file: "ggml-base.en.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
            size: 147_964_211,
            sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        }],
    },
    ModelSpec {
        id: "whisper-small.en",
        kind: ModelKind::Speech,
        name: "Whisper Small",
        blurb: "More accurate, slower on older CPUs",
        files: &[ModelFile {
            file: "ggml-small.en.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
            size: 487_614_201,
            sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
        }],
    },
    ModelSpec {
        id: "whisper-medium.en",
        kind: ModelKind::Speech,
        name: "Whisper Medium",
        blurb: "Most accurate, needs a fast CPU",
        files: &[ModelFile {
            file: "ggml-medium.en.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
            size: 1_533_774_781,
            sha256: "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
        }],
    },
    ModelSpec {
        id: "qwen2.5-0.5b",
        kind: ModelKind::Language,
        name: "Qwen2.5 0.5B",
        blurb: "Small and quick, runs on any CPU",
        files: &[
            ModelFile {
                file: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
                url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
                size: 491_400_032,
                sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
            },
            QWEN_TOKENIZER,
        ],
    },
    ModelSpec {
        id: "qwen2.5-1.5b",
        kind: ModelKind::Language,
        name: "Qwen2.5 1.5B",
        blurb: "Smarter splitting and matching, ~3x slower",
        files: &[
            ModelFile {
                file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
                url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
                size: 1_117_320_736,
                sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
            },
            QWEN_TOKENIZER,
        ],
    },
];

pub const DEFAULT_SPEECH_MODEL: &str = "whisper-base.en";
pub const DEFAULT_LANGUAGE_MODEL: &str = "qwen2.5-0.5b";

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOG.iter().find(|m| m.id == id)
}

pub fn of_kind(kind: ModelKind) -> impl Iterator<Item = &'static ModelSpec> {
    CATALOG.iter().filter(move |m| m.kind == kind)
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("flowstate")
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

pub fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

/// Live progress of a background download, shared with the UI.
#[derive(Default)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
}

pub type SharedProgress = Arc<Mutex<DownloadProgress>>;

/// Start downloading every missing file of `spec` on a background thread.
/// `on_update` is called whenever progress changes (used to wake the UI).
pub fn start_download(
    spec: &'static ModelSpec,
    on_update: impl Fn() + Send + 'static,
) -> SharedProgress {
    let progress = Arc::new(Mutex::new(DownloadProgress {
        model_id: spec.id.to_string(),
        total: spec.total_size(),
        ..Default::default()
    }));
    let shared = progress.clone();

    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(download_all(spec, &shared, &on_update)));

        let mut p = shared.lock().unwrap();
        if let Err(e) = result {
            p.error = Some(e);
        }
        p.finished = true;
        drop(p);
        on_update();
    });

    progress
}

async fn download_all(
    spec: &'static ModelSpec,
    progress: &SharedProgress,
    on_update: &(impl Fn() + Send),
) -> Result<(), String> {
    let dir = models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create models folder: {e}"))?;
    let client = reqwest::Client::new();
    let mut done_before: u64 = 0;

    for file in spec.files {
        let dest = dir.join(file.file);
        if std::fs::metadata(&dest).map(|m| m.len() == file.size).unwrap_or(false) {
            done_before += file.size;
            continue;
        }

        let tmp = dir.join(format!("{}.part", file.file));
        let response = client
            .get(file.url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("Download failed: HTTP {}", response.status()));
        }

        let mut out = std::fs::File::create(&tmp).map_err(|e| format!("Cannot write file: {e}"))?;
        let mut hasher = Sha256::new();
        let mut received: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut last_report = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download interrupted: {e}"))?;
            received += chunk.len() as u64;
            if received > file.size {
                let _ = std::fs::remove_file(&tmp);
                return Err("Download is larger than expected; aborted".into());
            }
            hasher.update(&chunk);
            out.write_all(&chunk).map_err(|e| format!("Cannot write file: {e}"))?;

            if last_report.elapsed().as_millis() > 100 {
                progress.lock().unwrap().downloaded = done_before + received;
                on_update();
                last_report = std::time::Instant::now();
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out);

        let digest = format!("{:x}", hasher.finalize());
        if received != file.size || digest != file.sha256 {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("{} failed verification; please retry", file.file));
        }
        std::fs::rename(&tmp, &dest).map_err(|e| format!("Cannot install model: {e}"))?;
        done_before += file.size;
        progress.lock().unwrap().downloaded = done_before;
        on_update();
    }
    Ok(())
}
