class Weave < Formula
  desc "Entity-level semantic merge driver for Git — resolves conflicts by understanding code structure"
  homepage "https://github.com/Ataraxy-Labs/weave"
  url "https://github.com/Ataraxy-Labs/weave/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "a98a1fa836a4cfd5c05a374182917bce374bcef55374560893393c62150c95e3"
  license "MIT"
  head "https://github.com/Ataraxy-Labs/weave.git", branch: "main"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on "rust" => :build
  depends_on "pkg-config" => :build

  def install
    cd "crates" do
      system "cargo", "install", *std_cargo_args(path: "weave-cli")
      system "cargo", "install", *std_cargo_args(path: "weave-driver")
      system "cargo", "install", *std_cargo_args(path: "weave-mcp")
    end
  end

  test do
    # Test that weave-cli can run the benchmark
    output = shell_output("#{bin}/weave-cli bench 2>&1")
    assert_match "weave merge benchmark", output
    assert_match "clean merges", output

    # Test that weave-driver binary exists and runs
    output = shell_output("#{bin}/weave-driver --help 2>&1", 1)
    assert_match "weave-driver", output.downcase
  end
end
