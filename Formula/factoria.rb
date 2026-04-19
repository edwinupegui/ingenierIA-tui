# Homebrew formula for ingenierIA TUI
# Install: brew install your-org/tap/ingenieria
# Or: brew tap your-org/tap && brew install ingenieria

class Ingenieria < Formula
  desc     "Terminal UI for ingenierIA MCP Server"
  homepage "https://github.com/your-org/ingenieria-tui"
  version  "0.7.1"
  license  "MIT"

  on_macos do
    on_arm do
      url "https://github.com/your-org/ingenieria-tui/releases/download/tui-v#{version}/ingenieria-aarch64-apple-darwin.tar.gz"
      # sha256 will be filled automatically by the release workflow
      sha256 "PLACEHOLDER"

      def install
        bin.install "ingenieria"
      end
    end

    on_intel do
      url "https://github.com/your-org/ingenieria-tui/releases/download/tui-v#{version}/ingenieria-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"

      def install
        bin.install "ingenieria"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/your-org/ingenieria-tui/releases/download/tui-v#{version}/ingenieria-x86_64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER"

      def install
        bin.install "ingenieria"
      end
    end
  end

  test do
    assert_match "ingenieria #{version}", shell_output("#{bin}/ingenieria --version")
  end
end
