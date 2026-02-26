# typed: false
# frozen_string_literal: true

# This formula is auto-updated by the release workflow.
# Manual edits will be overwritten on next release.
class Browsercli < Formula
  desc "A browser visual workspace for AI agents"
  homepage "https://github.com/justinhuangcode/browsercli"
  license "MIT"
  version "1.0.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/justinhuangcode/browsercli/releases/download/v#{version}/browsercli-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/justinhuangcode/browsercli/releases/download/v#{version}/browsercli-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/justinhuangcode/browsercli/releases/download/v#{version}/browsercli-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/justinhuangcode/browsercli/releases/download/v#{version}/browsercli-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "browsercli"
  end

  test do
    assert_match "browsercli", shell_output("#{bin}/browsercli --version")
  end
end
