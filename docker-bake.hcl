variable "REGISTRY" {
  default = "ghcr.io/fdarkshadow/meridian-core"
}

variable "TAG" {
  default = "dev"
}

group "default" {
  targets = ["core"]
}

target "core" {
  context    = "."
  dockerfile = "docker/core.Dockerfile"
  tags       = ["${REGISTRY}/core:${TAG}"]
  platforms  = ["linux/amd64", "linux/arm64"]
}
