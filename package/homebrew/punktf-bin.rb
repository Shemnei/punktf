class Punktf < Formula
  desc "A cross-platform multi-target dotfiles manager"
  homepage "https://github.com/Shemnei/punktf"
  version "v1.0.1"
  url "https://github.com/Shemnei/punktf/releases/download/#{version}/punktf-x86_64-unknown-linux-musl.tar.gz"
  sha256 "e67fe62cb03ae62c8b5cddff0d602700aa02e555d3f00b254794d5d13f59aba3"

  conflicts_with "punktf"

  def install
    bin.install "punktf"
  end
end
