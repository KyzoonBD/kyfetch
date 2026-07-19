class Kyfetch < Formula
  desc "Simple internal-URL crawler (mini Screaming Frog)"
  homepage "https://github.com/KyzoonBD/kyfetch"
  url "https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.4.0.tar.gz"
  sha256 "3a68f432cd2d4762a6806c7a96fdb1871c7e4ab966d6d33f5199ac7f5e923b9c"
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
