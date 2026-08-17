class Supazip < Formula
  desc "Cross-platform archive manager for 7z and ZIP, written in Rust"
  homepage "https://github.com/your-org/supazip"
  version "1.0.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_intel do
      url "https://github.com/your-org/supazip/releases/download/v#{version}/supazip-cli-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
    on_arm do
      url "https://github.com/your-org/supazip/releases/download/v#{version}/supazip-cli-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/your-org/supazip/releases/download/v#{version}/supazip-cli-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "supazip-cli" => "supazip"
    generate_completions_from_executable(bin/"supazip", "completions", shells: [:bash, :zsh, :fish])
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/supazip --version")
  end
end
