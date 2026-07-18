class Kyfetch < Formula
  desc "Simple internal-URL crawler (mini Screaming Frog)"
  homepage "https://github.com/KyzoonBD/kyfetch"
  url "https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "eb23f8aff5e3e867d297fb4bdb53adc95a100a32ae70ef21b539b96aba21651a"
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
