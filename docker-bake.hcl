group "default" {
  targets = ["base", "beardist"]
}

target "base" {
  context = "."
  dockerfile = "Dockerfile"
  target = "base"
  tags = ["ghcr.io/bearcove/base:latest"]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=registry"]
}

target "beardist" {
  context = "."
  dockerfile = "Dockerfile"
  args = {}
  tags = ["ghcr.io/bearcove/beardist:latest"]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=registry"]
}
