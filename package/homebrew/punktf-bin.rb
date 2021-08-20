class Punktf < Formula
  desc "A cross-platform multi-target dotfiles manager"
  homepage "https://github.com/Shemnei/punktf"
  version "v1.0.0"
  url "https://github.com/Shemnei/punktf/releases/download/#{version}/punktf-x86_64-unknown-linux-musl.tar.gz"
  sha256 "c4f9dc25df5a66e1bb914c9e69fac4ddb16ebb76196ce94bbce95cf8abdddc00"

  conflicts_with "punktf"

  def install
    bin.install "punktf"
  end
end
