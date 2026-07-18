class Kyfetch < Formula
  desc "Simple internal-URL crawler (mini Screaming Frog)"
  homepage "https://github.com/KyzoonBD/kyfetch"
  url "https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.3.1.tar.gz"
  sha256 "PENDING_AFTER_TAG"
  license "MIT"
  head "https://github.com/KyzoonBD/kyfetch.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "kyfetch", shell_output("#{bin}/kyfetch --help")
  end
end
