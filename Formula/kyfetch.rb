class Kyfetch < Formula
  desc "Simple internal-URL crawler (mini Screaming Frog)"
  homepage "https://github.com/KyzoonBD/kyfetch"
  url "https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.3.1.tar.gz"
  sha256 "665b1780ae97bb8ab052841fac4a304230ede4d23109c6beaf54eb03a877ecb8"
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
