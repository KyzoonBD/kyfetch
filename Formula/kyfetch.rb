class Kyfetch < Formula
  desc "Simple internal-URL crawler (mini Screaming Frog)"
  homepage "https://github.com/KyzoonBD/kyfetch"
  url "https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.5.0.tar.gz"
  sha256 "PLACEHOLDER"
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
