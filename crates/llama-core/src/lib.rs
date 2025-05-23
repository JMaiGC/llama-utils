//! Llama Core, abbreviated as `llama-core`, defines a set of APIs. Developers can utilize these APIs to build applications based on large models, such as chatbots, RAG, and more.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "logging")]
#[macro_use]
extern crate log;

pub mod audio;
pub mod chat;
pub mod completions;
pub mod embeddings;
pub mod error;
pub mod files;
pub mod graph;
pub mod images;
pub mod metadata;
pub mod models;
#[cfg(feature = "rag")]
#[cfg_attr(docsrs, doc(cfg(feature = "rag")))]
pub mod rag;
#[cfg(feature = "search")]
#[cfg_attr(docsrs, doc(cfg(feature = "search")))]
pub mod search;
pub mod tts;
pub mod utils;

pub use error::LlamaCoreError;
pub use graph::{EngineType, Graph, GraphBuilder};
#[cfg(feature = "whisper")]
use metadata::whisper::WhisperMetadata;
pub use metadata::{
    ggml::{GgmlMetadata, GgmlTtsMetadata},
    piper::PiperMetadata,
    BaseMetadata,
};
use once_cell::sync::OnceCell;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, RwLock},
};
use utils::{get_output_buffer, RunningMode};
use wasmedge_stable_diffusion::*;

// key: model_name, value: Graph
pub(crate) static CHAT_GRAPHS: OnceCell<Mutex<HashMap<String, Graph<GgmlMetadata>>>> =
    OnceCell::new();
// key: model_name, value: Graph
pub(crate) static EMBEDDING_GRAPHS: OnceCell<Mutex<HashMap<String, Graph<GgmlMetadata>>>> =
    OnceCell::new();
// key: model_name, value: Graph
pub(crate) static TTS_GRAPHS: OnceCell<Mutex<HashMap<String, Graph<GgmlTtsMetadata>>>> =
    OnceCell::new();
// cache bytes for decoding utf8
pub(crate) static CACHED_UTF8_ENCODINGS: OnceCell<Mutex<Vec<u8>>> = OnceCell::new();
// running mode
pub(crate) static RUNNING_MODE: OnceCell<RwLock<RunningMode>> = OnceCell::new();
// stable diffusion context for the text-to-image task
pub(crate) static SD_TEXT_TO_IMAGE: OnceCell<Mutex<TextToImage>> = OnceCell::new();
// stable diffusion context for the image-to-image task
pub(crate) static SD_IMAGE_TO_IMAGE: OnceCell<Mutex<ImageToImage>> = OnceCell::new();
// context for the audio task
#[cfg(feature = "whisper")]
pub(crate) static AUDIO_GRAPH: OnceCell<Mutex<Graph<WhisperMetadata>>> = OnceCell::new();
// context for the piper task
pub(crate) static PIPER_GRAPH: OnceCell<Mutex<Graph<PiperMetadata>>> = OnceCell::new();

pub(crate) const MAX_BUFFER_SIZE: usize = 2usize.pow(14) * 15 + 128;
pub(crate) const OUTPUT_TENSOR: usize = 0;
const PLUGIN_VERSION: usize = 1;

/// The directory for storing the archives in wasm virtual file system.
pub const ARCHIVES_DIR: &str = "archives";

/// Initialize the ggml context
pub fn init_ggml_chat_context(metadata_for_chats: &[GgmlMetadata]) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the core context");

    if metadata_for_chats.is_empty() {
        let err_msg = "The metadata for chat models is empty";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        return Err(LlamaCoreError::InitContext(err_msg.into()));
    }

    let mut chat_graphs = HashMap::new();
    for metadata in metadata_for_chats {
        let graph = Graph::new(metadata.clone())?;

        chat_graphs.insert(graph.name().to_string(), graph);
    }
    CHAT_GRAPHS.set(Mutex::new(chat_graphs)).map_err(|_| {
            let err_msg = "Failed to initialize the core context. Reason: The `CHAT_GRAPHS` has already been initialized";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            LlamaCoreError::InitContext(err_msg.into())
        })?;

    // set running mode
    let running_mode = RunningMode::CHAT;
    match RUNNING_MODE.get() {
        Some(mode) => {
            let mut mode = mode.write().unwrap();
            *mode |= running_mode;
        }
        None => {
            RUNNING_MODE.set(RwLock::new(running_mode)).map_err(|_| {
                let err_msg = "Failed to initialize the chat context. Reason: The `RUNNING_MODE` has already been initialized";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg.into())
            })?;
        }
    }

    Ok(())
}

/// Initialize the ggml context
pub fn init_ggml_embeddings_context(
    metadata_for_embeddings: &[GgmlMetadata],
) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the embeddings context");

    if metadata_for_embeddings.is_empty() {
        let err_msg = "The metadata for chat models is empty";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        return Err(LlamaCoreError::InitContext(err_msg.into()));
    }

    let mut embedding_graphs = HashMap::new();
    for metadata in metadata_for_embeddings {
        let graph = Graph::new(metadata.clone())?;

        embedding_graphs.insert(graph.name().to_string(), graph);
    }
    EMBEDDING_GRAPHS
            .set(Mutex::new(embedding_graphs))
            .map_err(|_| {
                let err_msg = "Failed to initialize the core context. Reason: The `EMBEDDING_GRAPHS` has already been initialized";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg.into())
            })?;

    // set running mode
    let running_mode = RunningMode::EMBEDDINGS;
    match RUNNING_MODE.get() {
        Some(mode) => {
            let mut mode = mode.write().unwrap();
            *mode |= running_mode;
        }
        None => {
            RUNNING_MODE.set(RwLock::new(running_mode)).map_err(|_| {
                let err_msg = "Failed to initialize the embeddings context. Reason: The `RUNNING_MODE` has already been initialized";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg.into())
            })?;
        }
    }

    Ok(())
}

/// Initialize the ggml context for RAG scenarios.
#[cfg(feature = "rag")]
pub fn init_ggml_rag_context(
    metadata_for_chats: &[GgmlMetadata],
    metadata_for_embeddings: &[GgmlMetadata],
) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the core context for RAG scenarios");

    // chat models
    if metadata_for_chats.is_empty() {
        let err_msg = "The metadata for chat models is empty";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        return Err(LlamaCoreError::InitContext(err_msg.into()));
    }
    let mut chat_graphs = HashMap::new();
    for metadata in metadata_for_chats {
        let graph = Graph::new(metadata.clone())?;

        chat_graphs.insert(graph.name().to_string(), graph);
    }
    CHAT_GRAPHS.set(Mutex::new(chat_graphs)).map_err(|_| {
        let err_msg = "Failed to initialize the core context. Reason: The `CHAT_GRAPHS` has already been initialized";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        LlamaCoreError::InitContext(err_msg.into())
    })?;

    // embedding models
    if metadata_for_embeddings.is_empty() {
        let err_msg = "The metadata for embeddings is empty";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        return Err(LlamaCoreError::InitContext(err_msg.into()));
    }
    let mut embedding_graphs = HashMap::new();
    for metadata in metadata_for_embeddings {
        let graph = Graph::new(metadata.clone())?;

        embedding_graphs.insert(graph.name().to_string(), graph);
    }
    EMBEDDING_GRAPHS
        .set(Mutex::new(embedding_graphs))
        .map_err(|_| {
            let err_msg = "Failed to initialize the core context. Reason: The `EMBEDDING_GRAPHS` has already been initialized";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            LlamaCoreError::InitContext(err_msg.into())
        })?;

    // set running mode
    let running_mode = RunningMode::RAG;
    match RUNNING_MODE.get() {
        Some(mode) => {
            let mut mode = mode.write().unwrap();
            *mode |= running_mode;
        }
        None => {
            RUNNING_MODE.set(RwLock::new(running_mode)).map_err(|_| {
                    let err_msg = "Failed to initialize the rag context. Reason: The `RUNNING_MODE` has already been initialized";

                    #[cfg(feature = "logging")]
                    error!(target: "stdout", "{err_msg}");

                    LlamaCoreError::InitContext(err_msg.into())
                })?;
        }
    }

    Ok(())
}

/// Initialize the ggml context for TTS scenarios.
pub fn init_ggml_tts_context(metadata_for_tts: &[GgmlTtsMetadata]) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the TTS context");

    if metadata_for_tts.is_empty() {
        let err_msg = "The metadata for tts models is empty";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        return Err(LlamaCoreError::InitContext(err_msg.into()));
    }

    let mut tts_graphs = HashMap::new();
    for metadata in metadata_for_tts {
        let graph = Graph::new(metadata.clone())?;

        tts_graphs.insert(graph.name().to_string(), graph);
    }
    TTS_GRAPHS.set(Mutex::new(tts_graphs)).map_err(|_| {
        let err_msg = "Failed to initialize the core context. Reason: The `TTS_GRAPHS` has already been initialized";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        LlamaCoreError::InitContext(err_msg.into())
    })?;

    // set running mode
    let running_mode = RunningMode::TTS;
    match RUNNING_MODE.get() {
        Some(mode) => {
            let mut mode = mode.write().unwrap();
            *mode |= running_mode;
        }
        None => {
            RUNNING_MODE.set(RwLock::new(running_mode)).map_err(|_| {
                let err_msg = "Failed to initialize the embeddings context. Reason: The `RUNNING_MODE` has already been initialized";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg.into())
            })?;
        }
    }

    Ok(())
}

/// Get the plugin info
///
/// Note that it is required to call `init_core_context` before calling this function.
pub fn get_plugin_info() -> Result<PluginInfo, LlamaCoreError> {
    #[cfg(feature = "logging")]
    debug!(target: "stdout", "Getting the plugin info");

    let running_mode = running_mode()?;

    if running_mode.contains(RunningMode::CHAT) || running_mode.contains(RunningMode::RAG) {
        let chat_graphs = match CHAT_GRAPHS.get() {
            Some(chat_graphs) => chat_graphs,
            None => {
                let err_msg = "Fail to get the underlying value of `CHAT_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        let chat_graphs = chat_graphs.lock().map_err(|e| {
            let err_msg = format!("Fail to acquire the lock of `CHAT_GRAPHS`. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        let graph = match chat_graphs.values().next() {
            Some(graph) => graph,
            None => {
                let err_msg = "Fail to get the underlying value of `CHAT_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        get_plugin_info_by_graph(graph)
    } else if running_mode.contains(RunningMode::EMBEDDINGS) {
        let embedding_graphs = match EMBEDDING_GRAPHS.get() {
            Some(embedding_graphs) => embedding_graphs,
            None => {
                let err_msg = "Fail to get the underlying value of `EMBEDDING_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        let embedding_graphs = embedding_graphs.lock().map_err(|e| {
            let err_msg = format!("Fail to acquire the lock of `EMBEDDING_GRAPHS`. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        let graph = match embedding_graphs.values().next() {
            Some(graph) => graph,
            None => {
                let err_msg = "Fail to get the underlying value of `EMBEDDING_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        get_plugin_info_by_graph(graph)
    } else if running_mode.contains(RunningMode::TTS) {
        let tts_graphs = match TTS_GRAPHS.get() {
            Some(tts_graphs) => tts_graphs,
            None => {
                let err_msg = "Fail to get the underlying value of `TTS_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        let tts_graphs = tts_graphs.lock().map_err(|e| {
            let err_msg = format!("Fail to acquire the lock of `TTS_GRAPHS`. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        let graph = match tts_graphs.values().next() {
            Some(graph) => graph,
            None => {
                let err_msg = "Fail to get the underlying value of `TTS_GRAPHS`.";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        };

        get_plugin_info_by_graph(graph)
    } else {
        let err_msg = "RUNNING_MODE is not set";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        Err(LlamaCoreError::Operation(err_msg.into()))
    }
}

fn get_plugin_info_by_graph<M: BaseMetadata + serde::Serialize + Clone + Default>(
    graph: &Graph<M>,
) -> Result<PluginInfo, LlamaCoreError> {
    #[cfg(feature = "logging")]
    debug!(target: "stdout", "Getting the plugin info by the graph named {}", graph.name());

    // get the plugin metadata
    let output_buffer = get_output_buffer(graph, PLUGIN_VERSION)?;
    let metadata: serde_json::Value = serde_json::from_slice(&output_buffer[..]).map_err(|e| {
        let err_msg = format!("Fail to deserialize the plugin metadata. {e}");

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{}", &err_msg);

        LlamaCoreError::Operation(err_msg)
    })?;

    // get build number of the plugin
    let plugin_build_number = match metadata.get("llama_build_number") {
        Some(value) => match value.as_u64() {
            Some(number) => number,
            None => {
                let err_msg = "Failed to convert the build number of the plugin to u64";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        },
        None => {
            let err_msg = "Metadata does not have the field `llama_build_number`.";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            return Err(LlamaCoreError::Operation(err_msg.into()));
        }
    };

    // get commit id of the plugin
    let plugin_commit = match metadata.get("llama_commit") {
        Some(value) => match value.as_str() {
            Some(commit) => commit,
            None => {
                let err_msg = "Failed to convert the commit id of the plugin to string";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::Operation(err_msg.into()));
            }
        },
        None => {
            let err_msg = "Metadata does not have the field `llama_commit`.";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            return Err(LlamaCoreError::Operation(err_msg.into()));
        }
    };

    #[cfg(feature = "logging")]
    debug!(target: "stdout", "Plugin info: b{plugin_build_number}(commit {plugin_commit})");

    Ok(PluginInfo {
        build_number: plugin_build_number,
        commit_id: plugin_commit.to_string(),
    })
}

/// Version info of the `wasi-nn_ggml` plugin, including the build number and the commit id.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub build_number: u64,
    pub commit_id: String,
}
impl std::fmt::Display for PluginInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "wasinn-ggml plugin: b{}(commit {})",
            self.build_number, self.commit_id
        )
    }
}

/// Return the current running mode.
pub fn running_mode() -> Result<RunningMode, LlamaCoreError> {
    #[cfg(feature = "logging")]
    debug!(target: "stdout", "Get the running mode.");

    match RUNNING_MODE.get() {
        Some(mode) => match mode.read() {
            Ok(mode) => Ok(*mode),
            Err(e) => {
                let err_msg = format!("Fail to get the underlying value of `RUNNING_MODE`. {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                Err(LlamaCoreError::Operation(err_msg))
            }
        },
        None => {
            let err_msg = "Fail to get the underlying value of `RUNNING_MODE`.";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            Err(LlamaCoreError::Operation(err_msg.into()))
        }
    }
}

/// Initialize the stable-diffusion context with the given full diffusion model
///
/// # Arguments
///
/// * `model_file` - Path to the stable diffusion model file.
///
/// * `lora_model_dir` - Path to the Lora model directory.
///
/// * `controlnet_path` - Path to the controlnet model file.
///
/// * `controlnet_on_cpu` - Whether to run the controlnet on CPU.
///
/// * `clip_on_cpu` - Whether to run the CLIP on CPU.
///
/// * `vae_on_cpu` - Whether to run the VAE on CPU.
///
/// * `n_threads` - Number of threads to use.
///
/// * `task` - The task type to perform.
#[allow(clippy::too_many_arguments)]
pub fn init_sd_context_with_full_model(
    model_file: impl AsRef<str>,
    lora_model_dir: Option<&str>,
    controlnet_path: Option<&str>,
    controlnet_on_cpu: bool,
    clip_on_cpu: bool,
    vae_on_cpu: bool,
    n_threads: i32,
    task: StableDiffusionTask,
) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the stable diffusion context with the full model");

    let control_net_on_cpu = match controlnet_path {
        Some(path) if !path.is_empty() => controlnet_on_cpu,
        _ => false,
    };

    // create the stable diffusion context for the text-to-image task
    if task == StableDiffusionTask::Full || task == StableDiffusionTask::TextToImage {
        let sd = SDBuidler::new(Task::TextToImage, model_file.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_lora_model_dir(lora_model_dir.unwrap_or_default())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .use_control_net(controlnet_path.unwrap_or_default(), control_net_on_cpu)
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .clip_on_cpu(clip_on_cpu)
            .vae_on_cpu(vae_on_cpu)
            .with_n_threads(n_threads)
            .build();

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd: {:?}", &sd);

        let ctx = sd.create_context().map_err(|e| {
            let err_msg = format!("Fail to create the context. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::InitContext(err_msg)
        })?;

        let ctx = match ctx {
            Context::TextToImage(ctx) => ctx,
            _ => {
                let err_msg = "Fail to get the context for the text-to-image task";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::InitContext(err_msg.into()));
            }
        };

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd text_to_image context: {:?}", &ctx);

        SD_TEXT_TO_IMAGE.set(Mutex::new(ctx)).map_err(|_| {
        let err_msg = "Failed to initialize the stable diffusion context. Reason: The `SD_TEXT_TO_IMAGE` has already been initialized";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        LlamaCoreError::InitContext(err_msg.into())
    })?;

        #[cfg(feature = "logging")]
        info!(target: "stdout", "The stable diffusion text-to-image context has been initialized");
    }

    // create the stable diffusion context for the image-to-image task
    if task == StableDiffusionTask::Full || task == StableDiffusionTask::ImageToImage {
        let sd = SDBuidler::new(Task::ImageToImage, model_file.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_lora_model_dir(lora_model_dir.unwrap_or_default())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .use_control_net(controlnet_path.unwrap_or_default(), control_net_on_cpu)
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .clip_on_cpu(clip_on_cpu)
            .vae_on_cpu(vae_on_cpu)
            .with_n_threads(n_threads)
            .build();

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd: {:?}", &sd);

        let ctx = sd.create_context().map_err(|e| {
            let err_msg = format!("Fail to create the context. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::InitContext(err_msg)
        })?;

        let ctx = match ctx {
            Context::ImageToImage(ctx) => ctx,
            _ => {
                let err_msg = "Fail to get the context for the image-to-image task";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::InitContext(err_msg.into()));
            }
        };

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd image_to_image context: {:?}", &ctx);

        SD_IMAGE_TO_IMAGE.set(Mutex::new(ctx)).map_err(|_| {
            let err_msg = "Failed to initialize the stable diffusion context. Reason: The `SD_IMAGE_TO_IMAGE` has already been initialized";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            LlamaCoreError::InitContext(err_msg.into())
        })?;

        #[cfg(feature = "logging")]
        info!(target: "stdout", "The stable diffusion image-to-image context has been initialized");
    }

    Ok(())
}

/// Initialize the stable-diffusion context with the given standalone diffusion model
///
/// # Arguments
///
/// * `model_file` - Path to the standalone diffusion model file.
///
/// * `vae` - Path to the VAE model file.
///
/// * `clip_l` - Path to the CLIP model file.
///
/// * `t5xxl` - Path to the T5-XXL model file.
///
/// * `lora_model_dir` - Path to the Lora model directory.
///
/// * `controlnet_path` - Path to the controlnet model file.
///
/// * `controlnet_on_cpu` - Whether to run the controlnet on CPU.
///
/// * `clip_on_cpu` - Whether to run the CLIP on CPU.
///
/// * `vae_on_cpu` - Whether to run the VAE on CPU.
///
/// * `n_threads` - Number of threads to use.
///
/// * `task` - The task type to perform.
#[allow(clippy::too_many_arguments)]
pub fn init_sd_context_with_standalone_model(
    model_file: impl AsRef<str>,
    vae: impl AsRef<str>,
    clip_l: impl AsRef<str>,
    t5xxl: impl AsRef<str>,
    lora_model_dir: Option<&str>,
    controlnet_path: Option<&str>,
    controlnet_on_cpu: bool,
    clip_on_cpu: bool,
    vae_on_cpu: bool,
    n_threads: i32,
    task: StableDiffusionTask,
) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the stable diffusion context with the standalone diffusion model");

    let control_net_on_cpu = match controlnet_path {
        Some(path) if !path.is_empty() => controlnet_on_cpu,
        _ => false,
    };

    // create the stable diffusion context for the text-to-image task
    if task == StableDiffusionTask::Full || task == StableDiffusionTask::TextToImage {
        let sd = SDBuidler::new_with_standalone_model(Task::TextToImage, model_file.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_vae_path(vae.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_clip_l_path(clip_l.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_t5xxl_path(t5xxl.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_lora_model_dir(lora_model_dir.unwrap_or_default())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .use_control_net(controlnet_path.unwrap_or_default(), control_net_on_cpu)
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .clip_on_cpu(clip_on_cpu)
            .vae_on_cpu(vae_on_cpu)
            .with_n_threads(n_threads)
            .build();

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd: {:?}", &sd);

        let ctx = sd.create_context().map_err(|e| {
            let err_msg = format!("Fail to create the context. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::InitContext(err_msg)
        })?;

        let ctx = match ctx {
            Context::TextToImage(ctx) => ctx,
            _ => {
                let err_msg = "Fail to get the context for the text-to-image task";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::InitContext(err_msg.into()));
            }
        };

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd text_to_image context: {:?}", &ctx);

        SD_TEXT_TO_IMAGE.set(Mutex::new(ctx)).map_err(|_| {
            let err_msg = "Failed to initialize the stable diffusion context. Reason: The `SD_TEXT_TO_IMAGE` has already been initialized";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            LlamaCoreError::InitContext(err_msg.into())
        })?;

        #[cfg(feature = "logging")]
        info!(target: "stdout", "The stable diffusion text-to-image context has been initialized");
    }

    // create the stable diffusion context for the image-to-image task
    if task == StableDiffusionTask::Full || task == StableDiffusionTask::ImageToImage {
        let sd = SDBuidler::new_with_standalone_model(Task::ImageToImage, model_file.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_vae_path(vae.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_clip_l_path(clip_l.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_t5xxl_path(t5xxl.as_ref())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .with_lora_model_dir(lora_model_dir.unwrap_or_default())
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .use_control_net(controlnet_path.unwrap_or_default(), control_net_on_cpu)
            .map_err(|e| {
                let err_msg =
                    format!("Failed to initialize the stable diffusion context. Reason: {e}");

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg)
            })?
            .clip_on_cpu(clip_on_cpu)
            .vae_on_cpu(vae_on_cpu)
            .with_n_threads(n_threads)
            .build();

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd: {:?}", &sd);

        let ctx = sd.create_context().map_err(|e| {
            let err_msg = format!("Fail to create the context. {e}");

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{}", &err_msg);

            LlamaCoreError::InitContext(err_msg)
        })?;

        let ctx = match ctx {
            Context::ImageToImage(ctx) => ctx,
            _ => {
                let err_msg = "Fail to get the context for the image-to-image task";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                return Err(LlamaCoreError::InitContext(err_msg.into()));
            }
        };

        #[cfg(feature = "logging")]
        info!(target: "stdout", "sd image_to_image context: {:?}", &ctx);

        SD_IMAGE_TO_IMAGE.set(Mutex::new(ctx)).map_err(|_| {
        let err_msg = "Failed to initialize the stable diffusion context. Reason: The `SD_IMAGE_TO_IMAGE` has already been initialized";

        #[cfg(feature = "logging")]
        error!(target: "stdout", "{err_msg}");

        LlamaCoreError::InitContext(err_msg.into())
    })?;

        #[cfg(feature = "logging")]
        info!(target: "stdout", "The stable diffusion image-to-image context has been initialized");
    }

    Ok(())
}

/// The task type of the stable diffusion context
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum StableDiffusionTask {
    /// `text_to_image` context
    TextToImage,
    /// `image_to_image` context
    ImageToImage,
    /// Both `text_to_image` and `image_to_image` contexts
    Full,
}

/// Initialize the whisper context
#[cfg(feature = "whisper")]
pub fn init_whisper_context(whisper_metadata: &WhisperMetadata) -> Result<(), LlamaCoreError> {
    // create and initialize the audio context
    let graph = GraphBuilder::new(EngineType::Whisper)?
        .with_config(whisper_metadata.clone())?
        .use_cpu()
        .build_from_files([&whisper_metadata.model_path])?;

    match AUDIO_GRAPH.get() {
        Some(mutex_graph) => {
            #[cfg(feature = "logging")]
            info!(target: "stdout", "Re-initialize the audio context");

            match mutex_graph.lock() {
                Ok(mut locked_graph) => *locked_graph = graph,
                Err(e) => {
                    let err_msg = format!("Failed to lock the graph. Reason: {e}");

                    #[cfg(feature = "logging")]
                    error!(target: "stdout", "{err_msg}");

                    return Err(LlamaCoreError::InitContext(err_msg));
                }
            }
        }
        None => {
            #[cfg(feature = "logging")]
            info!(target: "stdout", "Initialize the audio context");

            AUDIO_GRAPH.set(Mutex::new(graph)).map_err(|_| {
                let err_msg = "Failed to initialize the audio context. Reason: The `AUDIO_GRAPH` has already been initialized";

                #[cfg(feature = "logging")]
                error!(target: "stdout", "{err_msg}");

                LlamaCoreError::InitContext(err_msg.into())
            })?;
        }
    }

    #[cfg(feature = "logging")]
    info!(target: "stdout", "The audio context has been initialized");

    Ok(())
}

/// Initialize the piper context
///
/// # Arguments
///
/// * `voice_model` - Path to the voice model file.
///
/// * `voice_config` - Path to the voice config file.
///
/// * `espeak_ng_data` - Path to the espeak-ng data directory.
///
pub fn init_piper_context(
    piper_metadata: &PiperMetadata,
    voice_model: impl AsRef<Path>,
    voice_config: impl AsRef<Path>,
    espeak_ng_data: impl AsRef<Path>,
) -> Result<(), LlamaCoreError> {
    #[cfg(feature = "logging")]
    info!(target: "stdout", "Initializing the piper context");

    let config = serde_json::json!({
        "model": voice_model.as_ref().to_owned(),
        "config": voice_config.as_ref().to_owned(),
        "espeak_data": espeak_ng_data.as_ref().to_owned(),
    });

    // create and initialize the audio context
    let graph = GraphBuilder::new(EngineType::Piper)?
        .with_config(piper_metadata.clone())?
        .use_cpu()
        .build_from_buffer([config.to_string()])?;

    PIPER_GRAPH.set(Mutex::new(graph)).map_err(|_| {
            let err_msg = "Failed to initialize the piper context. Reason: The `PIPER_GRAPH` has already been initialized";

            #[cfg(feature = "logging")]
            error!(target: "stdout", "{err_msg}");

            LlamaCoreError::InitContext(err_msg.into())
        })?;

    #[cfg(feature = "logging")]
    info!(target: "stdout", "The piper context has been initialized");

    Ok(())
}
