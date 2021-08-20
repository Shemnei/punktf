class Punktf < Formula
  desc "A cross-platform multi-target dotfiles manager"
  homepage "https://github.com/Shemnei/punktf"
  version "v1.0.0"
  url "https://github.com/Shemnei/punktf/releases/download/#{version}/punktf-x86_64-unknown-linux-musl.tar.gz"
  sha256 "88b9e22770c6dd1d44843e145b617f31347a17c47ff557385ac958df12a8d87d"

  conflicts_with "punktf"

  def install
    bin.install "punktf"
  end
end
