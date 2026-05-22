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
  download_source: ModelAssetSource;
  install_comfyui_relative_path: string;
}
