export type CustomNodeGitSource = {
  source_type: "git";
  repository_url: string;
  revision: string;
}

export type CustomNodeInstall = {
  comfyui_custom_nodes_relative_path: string;
  python_requirements_path: string;
}

export type CustomNode = {
  id: string;
  name: string;
  git_source: CustomNodeGitSource;
  install: CustomNodeInstall;
}
