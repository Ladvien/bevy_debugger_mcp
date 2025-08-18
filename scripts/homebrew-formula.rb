class BevyDebuggerMcp < Formula
  desc "MCP server for debugging Bevy games with Claude"
  homepage "https://github.com/ladvien/bevy_debugger_mcp"
  version "0.1.0"
  license "GPL-3.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/ladvien/bevy_debugger_mcp/releases/download/v#{version}/bevy-debugger-mcp-aarch64-apple-darwin.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    else
      url "https://github.com/ladvien/bevy_debugger_mcp/releases/download/v#{version}/bevy-debugger-mcp-x86_64-apple-darwin.tar.gz"
      sha256 "YOUR_SHA256_HERE"
    end
  end

  on_linux do
    url "https://github.com/ladvien/bevy_debugger_mcp/releases/download/v#{version}/bevy-debugger-mcp-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "YOUR_SHA256_HERE"
  end

  def install
    bin.install "bevy-debugger-mcp"
  end

  def post_install
    # Create config directory
    config_dir = Pathname.new(Dir.home) / ".config" / "bevy-debugger"
    config_dir.mkpath

    # Create default config if it doesn't exist
    config_file = config_dir / "config.toml"
    unless config_file.exist?
      config_file.write <<~EOS
        [connection]
        bevy_host = "localhost"
        bevy_port = 15702
        mcp_port = 3000

        [logging]
        level = "info"
        debug_mode = false

        [features]
        auto_reconnect = true
        checkpoint_interval = 1000
        max_recording_size = "100MB"
      EOS
    end
  end

  def caveats
    <<~EOS
      ✨ Bevy Debugger MCP Server has been installed!

      To set up Claude Desktop integration:
        bevy-debugger-mcp setup-claude

      To start the server:
        bevy-debugger-mcp serve

      Configuration file is located at:
        ~/.config/bevy-debugger/config.toml

      Don't forget to add RemotePlugin to your Bevy game!
    EOS
  end

  service do
    run [opt_bin/"bevy-debugger-mcp", "serve"]
    keep_alive true
    log_path var/"log/bevy-debugger-mcp.log"
    error_log_path var/"log/bevy-debugger-mcp-error.log"
    environment_variables RUST_LOG: "info"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/bevy-debugger-mcp --version")
    assert_match "doctor", shell_output("#{bin}/bevy-debugger-mcp --help")
  end
end