class Bgone < Formula
  desc "Ultra-fast CLI tool for removing solid background colors from images"
  homepage "https://github.com/benface/bgone"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/benface/bgone/releases/download/v0.1.0/bgone-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "c51f781c0c2134a2dae6423360a08cba6bf8096f3856186f7d75e942703aea93"
    else
      url "https://github.com/benface/bgone/releases/download/v0.1.0/bgone-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "042dfd2474e515b83eb9b6fcba234a70884ca7938b87ca460ae60cc121eb0566"
    end
  end

  def install
    bin.install "bgone"
  end

  test do
    system "#{bin}/bgone", "--version"
  end
end