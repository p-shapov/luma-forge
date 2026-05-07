export type ModelAssetKind =
  | "checkpoint"
  | "diffusion_model"
  | "vae"
  | "text_encoder"
  | "clip"
  | "clip_vision"
  | "lora"
  | "controlnet"
  | "upscaler"
  | "embedding"
  | "other";

export type HuggingFaceModelAssetSource = {
  source_type: "huggingface";
  repository_id: string;
  file_path: string;
  revision: string;
}

export type ModelAssetSource =
  | HuggingFaceModelAssetSource;

export type ModelAsset = {
  id: string;
  name: string;
  model_asset_kind: ModelAssetKind;
  file_size_bytes: number;
  download_source: ModelAssetSource;
}
